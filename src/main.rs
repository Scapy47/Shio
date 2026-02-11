use clap::Parser;
use nucleo_matcher::{
    Config, Matcher, Utf32Str,
    pattern::{AtomKind, CaseMatching, Normalization, Pattern},
};
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event},
    layout::{Constraint, HorizontalAlignment, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, BorderType, Paragraph, Row, Table, TableState},
};
use std::{
    process::Command,
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};
use tui_input::{Input, backend::crossterm::EventHandler};

mod api;
mod utils;
use crate::{
    api::{AnimeEdge, Api, Mode},
    utils::{ASCII_ART, decrypt_url},
};

#[derive(Parser, Debug)]
struct Args {
    /// Name of the anime to search
    name: String,

    /// Print debuging info
    #[arg(long)]
    debug: bool,
}

#[derive(Debug)]
enum ApiResponse {
    Error(String),
    SearchResp(Vec<AnimeEdge>),
    EpisodeListResp((String, Vec<String>, String)),
    EpisodeLinksResp((String, Vec<(String, String)>)),
}

#[derive(Debug)]
/// View of the app
enum View {
    /// show loading layout
    Loading,
    /// select the anime
    Search,
    /// select episode
    Episode,
    /// select provider
    Provider,
}

#[derive(Debug)]
struct App {
    /// current view that is being rendered
    view: View,
    /// condition that is allowing loop to contine
    exit: bool,
    /// arguments passed to cli
    args: Args,
    /// search bar input state
    input: Input,
    api: Arc<Api>,
    resp: ApiResponse,
    matcher: Matcher,
    rows_to_data_index: Vec<usize>,
    table_state: TableState,
}

impl App {
    fn new() -> Self {
        let args = Args::parse();
        let api = Arc::new(Api::new(Mode::Sub, args.debug));

        Self {
            table_state: TableState::default(),
            args: args,
            input: Input::default(),
            api: api,
            matcher: Matcher::new(Config::DEFAULT),
            rows_to_data_index: Vec::new(),
            resp: ApiResponse::Error("Error-001: Application has just initialized".to_string()),
            exit: false,
            view: View::Loading,
        }
    }

    fn main_loop(&mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        let (tx, rx) = mpsc::channel::<ApiResponse>();

        let api_clone = self.api.clone();
        let name = self.args.name.clone();
        let tx_clone = tx.clone();
        thread::spawn(move || match api_clone.search_anime(name) {
            Ok(resp) => tx_clone.send(ApiResponse::SearchResp(resp.data.shows.edges)),
            Err(e) => tx_clone.send(ApiResponse::Error(e.to_string())),
        });

        self.table_state.select(Some(0));

        while !self.exit {
            if let Ok(api_resp) = rx.try_recv() {
                match &api_resp {
                    ApiResponse::SearchResp(resp) => {
                        self.rows_to_data_index = (0..resp.len()).collect();
                        self.view = View::Search;
                    }
                    ApiResponse::EpisodeListResp((_, ep_list, _)) => {
                        self.rows_to_data_index = (0..ep_list.len()).collect();
                        self.view = View::Episode;
                    }
                    ApiResponse::EpisodeLinksResp((_, links)) => {
                        self.rows_to_data_index = (0..links.len()).collect();
                        self.view = View::Provider;
                    }
                    ApiResponse::Error(e) => {
                        println!("{}", e);
                        self.exit = true
                    }
                }

                self.resp = api_resp;
            } else {
            }

            if self.rows_to_data_index.is_empty() {
                self.update_row_to_data_index();
            }

            terminal.draw(|frame| self.render(frame))?;

            if event::poll(Duration::from_millis(16))? {
                let event = event::read()?;
                if let Event::Key(key) = event {
                    match key.code {
                        event::KeyCode::Esc => return Ok(()),
                        event::KeyCode::Down => self.table_state.select_next(),
                        event::KeyCode::Up => self.table_state.select_previous(),
                        event::KeyCode::Left => self.table_state.select_next_column(),
                        event::KeyCode::Right => self.table_state.select_previous_column(),
                        event::KeyCode::Enter => match self.view {
                            View::Loading => (),
                            View::Search => {
                                if let ApiResponse::SearchResp(resp) = &self.resp {
                                    let Some(row) = self.table_state.selected() else {
                                        return Ok(());
                                    };
                                    let id = resp[self.rows_to_data_index[row]].id.clone();

                                    let tx_clone = tx.clone();
                                    let api_clone = self.api.clone();
                                    thread::spawn(move || match api_clone.get_episode_list(&id) {
                                        Ok(resp) => {
                                            tx_clone.send(ApiResponse::EpisodeListResp(resp))
                                        }
                                        Err(e) => tx_clone.send(ApiResponse::Error(e.to_string())),
                                    });
                                }
                            }
                            View::Episode => {
                                if let ApiResponse::EpisodeListResp((_, list, id)) = &self.resp {
                                    let Some(row) = self.table_state.selected() else {
                                        return Ok(());
                                    };
                                    let ep = list[self.rows_to_data_index[row]].clone();
                                    let id_clone = id.clone();
                                    let tx_clone = tx.clone();
                                    let api_clone = self.api.clone();
                                    thread::spawn(move || {
                                        match api_clone.get_episode_links(&id_clone, &ep) {
                                            Ok(resp) => {
                                                tx_clone.send(ApiResponse::EpisodeLinksResp(resp))
                                            }
                                            Err(e) => {
                                                tx_clone.send(ApiResponse::Error(e.to_string()))
                                            }
                                        }
                                    });
                                }
                            }
                            View::Provider => {
                                if let ApiResponse::EpisodeLinksResp((_, links)) = &self.resp {
                                    let Some(row) = self.table_state.selected() else {
                                        return Ok(());
                                    };
                                    let (_provider, url) = &links[self.rows_to_data_index[row]];
                                    let api = self.api.clone();

                                    let cmd = Command::new("curl")
                                        .arg("-L")
                                        .arg("-H")
                                        .arg(format!("Referer: {}", api.referer))
                                        .arg("-H")
                                        .arg(format!("User-Agent: {}", api.user_agent))
                                        .arg(url)
                                        .arg("-O")
                                        .arg("--progress-bar")
                                        .status()
                                        .expect("Failed to execute curl")
                                        .code()
                                        .unwrap_or(1);

                                    if cmd == 1 || cmd == 0 {
                                        self.exit = true;
                                    }
                                }
                            }
                        },
                        _ => {
                            self.input.handle_event(&event);
                            self.update_row_to_data_index();
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// update the index of rows to data pointer vec
    fn update_row_to_data_index(&mut self) {
        let pattern = Pattern::new(
            &self.input.value(),
            CaseMatching::Smart,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );

        let mut buf = Vec::new();

        match &self.resp {
            ApiResponse::SearchResp(resp) => {
                let mut matches_result: Vec<(usize, u32)> = resp
                    .iter()
                    .enumerate()
                    .filter_map(|(og_index, item)| {
                        let haystack = Utf32Str::new(&item.name, &mut buf);

                        pattern
                            .score(haystack, &mut self.matcher)
                            .map(|score| (og_index, score))
                    })
                    .collect();
                matches_result.sort_by(|a, b| b.1.cmp(&a.1));
                self.rows_to_data_index = matches_result.into_iter().map(|(i, _)| i).collect();
            }
            ApiResponse::EpisodeListResp((_, ep_list, _)) => {
                let mut matches_result: Vec<(usize, u32)> = ep_list
                    .iter()
                    .enumerate()
                    .filter_map(|(og_index, item)| {
                        let haystack = Utf32Str::new(&item, &mut buf);

                        pattern
                            .score(haystack, &mut self.matcher)
                            .map(|score| (og_index, score))
                    })
                    .collect();
                matches_result.sort_by(|a, b| b.1.cmp(&a.1));
                self.rows_to_data_index = matches_result.into_iter().map(|(i, _)| i).collect();
            }
            ApiResponse::EpisodeLinksResp((_, links)) => {
                let mut matches_result: Vec<(usize, u32)> = links
                    .iter()
                    .enumerate()
                    .filter_map(|(og_index, (provider_name, _link))| {
                        let haystack = Utf32Str::new(&provider_name, &mut buf);

                        pattern
                            .score(haystack, &mut self.matcher)
                            .map(|score| (og_index, score))
                    })
                    .collect();
                matches_result.sort_by(|a, b| b.1.cmp(&a.1));
                self.rows_to_data_index = matches_result.into_iter().map(|(i, _)| i).collect();
            }
            ApiResponse::Error(_) => {
                return;
            }
        };
    }

    /// render the skeleton before data is there
    fn render_skeleton(&self, frame: &mut Frame) {
        frame.render_widget(
            Block::bordered().border_type(BorderType::Rounded),
            frame.area(),
        );
    }

    fn render_search_input(&self, frame: &mut Frame, area: Rect) {
        let width = area.width.max(2) - 2;
        let scroll = self.input.visual_scroll(width as usize);

        frame.render_widget(
            Paragraph::new(self.input.value()).block(
                Block::bordered()
                    .title("Search")
                    .title_style(Style::new().bold())
                    .title_alignment(HorizontalAlignment::Center)
                    .border_type(BorderType::Rounded)
                    .style(Style::new().cyan()),
            ),
            area,
        );

        let cursor_x = area.x + 1 + (self.input.visual_cursor().max(scroll) - scroll) as u16;
        let cursor_y = area.y + 1;

        frame.set_cursor_position((cursor_x, cursor_y));
    }

    fn render_search_result(&mut self, frame: &mut Frame, area: Rect) {
        let search_resp_vec = match &self.resp {
            ApiResponse::SearchResp(resp) => resp,
            _ => {
                return;
            }
        };

        let mut rows = vec![];
        for index in &self.rows_to_data_index {
            let item = &search_resp_vec[*index];

            let ep_count = item
                .available_episodes
                .as_ref()
                .and_then(|map| map.get("sub"))
                .and_then(|v| v.as_u64())
                .unwrap_or(0)
                .to_string();

            let english_name = item.english_name.as_deref().unwrap_or(&item.name);

            rows.push(
                Row::new(vec![
                    format!("{}\n{}", item.name, english_name),
                    ep_count,
                    item.id.clone(),
                    item.typename.clone(),
                ])
                .height(2),
            );
        }

        frame.render_stateful_widget(
            Table::new(
                rows,
                [
                    Constraint::Percentage(80),
                    Constraint::Percentage(5),
                    Constraint::Percentage(5),
                    Constraint::Fill(1),
                ],
            )
            .block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .title("Results")
                    .title_alignment(HorizontalAlignment::Center),
            )
            .highlight_symbol("» ")
            .row_highlight_style(Style::new().bg(Color::Cyan).fg(Color::Black)),
            area,
            &mut self.table_state,
        );
    }

    fn render_episode_list(&mut self, frame: &mut Frame, area: Rect, data: Vec<String>) {
        let mut rows = Vec::new();

        for index in &self.rows_to_data_index {
            let item = &data[*index];
            rows.push(Row::new(vec![item.clone()]));
        }

        frame.render_stateful_widget(
            Table::new(rows, [Constraint::Fill(1)])
                .highlight_symbol("» ")
                .row_highlight_style(Style::new().bg(Color::LightCyan).fg(Color::Black)),
            area,
            &mut self.table_state,
        );
    }

    fn render_episode_links(&mut self, frame: &mut Frame, area: Rect, data: Vec<(String, String)>) {
        let mut rows = Vec::new();

        for index in &self.rows_to_data_index {
            let (provider_name, link) = &data[*index];
            rows.push(Row::new(vec![provider_name.clone(), link.clone()]));
        }

        frame.render_stateful_widget(
            Table::new(rows, [Constraint::Percentage(10), Constraint::Fill(1)])
                .highlight_symbol("» ")
                .row_highlight_style(Style::new().bg(Color::LightCyan).fg(Color::Black)),
            area,
            &mut self.table_state,
        );
    }

    fn render_side_menu(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(Paragraph::new(ASCII_ART).centered(), area);
    }

    fn render(&mut self, frame: &mut Frame) {
        let [top, bottom] =
            Layout::vertical([Constraint::Percentage(10), Constraint::Fill(1)]).areas(frame.area());

        let [bottom_left, bottom_right] =
            Layout::horizontal([Constraint::Percentage(70), Constraint::Fill(1)]).areas(bottom);

        match self.view {
            View::Loading => self.render_skeleton(frame),
            View::Search => {
                if let ApiResponse::SearchResp(_) = &self.resp {
                    self.render_search_input(frame, top);
                    self.render_search_result(frame, bottom_left);
                    self.render_side_menu(frame, bottom_right);
                }
            }
            View::Episode => {
                if let ApiResponse::EpisodeListResp((_, ep_list, _)) = &self.resp {
                    self.render_search_input(frame, top);
                    self.render_episode_list(frame, bottom_left, ep_list.clone());
                    self.render_side_menu(frame, bottom_right);
                }
            }
            View::Provider => {
                if let ApiResponse::EpisodeLinksResp((_, links)) = &self.resp {
                    self.render_search_input(frame, top);
                    self.render_episode_links(frame, bottom_left, links.clone());
                }
            }
        }
    }
}

fn main() -> color_eyre::eyre::Result<()> {
    color_eyre::install()?;

    ratatui::run(|terminal| App::new().main_loop(terminal))?;
    Ok(())
}
