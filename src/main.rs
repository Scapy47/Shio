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
    env,
    process::Command,
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
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
    /// Name of the anime
    name: String,

    /// in which mode sub, dub or raw (whatever is available)
    #[arg(short)]
    mode: Option<Mode>,

    /// get debuging data
    #[arg(long)]
    debug: bool,
}

#[derive(Debug, Default)]
struct Resp {
    search: Option<Vec<AnimeEdge>>,
    episode_list: Option<(String, Vec<String>, String)>,
    episode_provider_list: Option<(String, Vec<(String, String)>)>,
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
    // select icon
    select_icon: String,
    /// current view that is being rendered
    view: View,
    /// condition that is allowing loop to contine
    exit: bool,
    /// arguments passed to cli
    args: Args,
    /// search bar input state
    input: Input,
    ///
    api: Arc<Api>,
    ///
    resp: Resp,
    ///
    matcher: Matcher,
    ///
    rows_to_data_index: Vec<usize>,
    ///
    table_state: TableState,
    //
    ui_loop_tick: Instant,
}

impl App {
    fn new() -> Self {
        let args = Args::parse();
        let mode = args.mode.unwrap_or(Mode::Sub);
        let api = Arc::new(Api::new(mode, args.debug));

        Self {
            select_icon: String::default(),
            table_state: TableState::default(),
            args: args,
            input: Input::default(),
            api,
            matcher: Matcher::new(Config::DEFAULT),
            rows_to_data_index: Vec::new(),
            exit: false,
            view: View::Loading,
            resp: Resp::default(),
            ui_loop_tick: Instant::now(),
        }
    }

    fn select_icon_animation(&mut self) {
        let icon_s1 = " => ".to_string();
        let icon_s2 = "    ".to_string();

        let now = Instant::now();

        if now.duration_since(self.ui_loop_tick) >= Duration::from_millis(300) {
            if self.select_icon.is_empty() || self.select_icon == icon_s2 {
                self.select_icon = icon_s1
            } else if self.select_icon == icon_s1 {
                self.select_icon = icon_s2
            }
            self.ui_loop_tick = now;
        }
    }

    fn main_loop(&mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        let (tx, rx) = mpsc::channel::<Option<Resp>>();

        let api_clone = self.api.clone();
        let name = self.args.name.clone();
        let tx_clone = tx.clone();
        thread::spawn(move || match api_clone.search_anime(name) {
            Ok(resp) => tx_clone.send(Some(Resp {
                search: Some(resp.data.shows.edges),
                ..Default::default()
            })),
            Err(e) => {
                let _ = tx_clone.send(None);
                panic!("Error getting search results: {}", e);
            }
        });

        while !self.exit {
            if let Ok(api_resp) = rx.try_recv() {
                if let Some(resp) = api_resp {
                    if let Some(search_resp) = resp.search {
                        self.rows_to_data_index = (0..search_resp.len()).collect();
                        self.resp.search = Some(search_resp);
                        self.table_state.select(Some(0));
                        self.view = View::Search
                    }
                    if let Some(ep_list_resp) = resp.episode_list {
                        self.rows_to_data_index = (0..ep_list_resp.1.len()).collect();
                        self.resp.episode_list = Some(ep_list_resp);
                        self.table_state.select(Some(0));
                        self.view = View::Episode
                    }
                    if let Some(ep_provider_list_resp) = resp.episode_provider_list {
                        self.rows_to_data_index = (0..ep_provider_list_resp.1.len()).collect();
                        self.resp.episode_provider_list = Some(ep_provider_list_resp);
                        self.table_state.select(Some(0));
                        self.view = View::Provider
                    }
                }
            }

            if self.rows_to_data_index.is_empty() {
                self.update_row_to_data_index();
            }

            terminal.draw(|frame| self.render(frame))?;

            self.select_icon_animation();

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
                                if let Some(resp) = &self.resp.search {
                                    let Some(row) = self.table_state.selected() else {
                                        return Ok(());
                                    };
                                    let id = resp[self.rows_to_data_index[row]].id.clone();

                                    let tx_clone = tx.clone();
                                    let api_clone = self.api.clone();
                                    thread::spawn(move || match api_clone.get_episode_list(&id) {
                                        Ok(resp) => tx_clone.send(Some(Resp {
                                            episode_list: Some(resp),
                                            ..Default::default()
                                        })),
                                        Err(e) => {
                                            let _ = tx_clone.send(None);
                                            panic!("Error getting episode list: {}", e);
                                        }
                                    });
                                }
                            }
                            View::Episode => {
                                if let Some((_, list, id)) = &self.resp.episode_list {
                                    let Some(row) = self.table_state.selected() else {
                                        return Ok(());
                                    };
                                    let ep = list[self.rows_to_data_index[row]].clone();
                                    let id_clone = id.clone();
                                    let tx_clone = tx.clone();
                                    let api_clone = self.api.clone();
                                    thread::spawn(move || {
                                        match api_clone.get_episode_links(&id_clone, &ep) {
                                            Ok(resp) => tx_clone.send(Some(Resp {
                                                episode_provider_list: Some(resp),
                                                ..Default::default()
                                            })),
                                            Err(e) => {
                                                let _ = tx_clone.send(None);
                                                panic!("Error getting episode links: {}", e);
                                            }
                                        }
                                    });
                                }
                            }
                            View::Provider => {
                                if let Some((_, links)) = &self.resp.episode_provider_list {
                                    let _ = terminal.clear();

                                    let Some(row) = self.table_state.selected() else {
                                        return Ok(());
                                    };
                                    let (_provider, url) = &links[self.rows_to_data_index[row]];
                                    let api = self.api.clone();

                                    let url = &(url.contains("clock.json")
                                        || url.contains("https://allanime.day"))
                                    .then(|| api.resolve_clock_urls(&url).unwrap())
                                    .unwrap_or(url.to_string());

                                    let default_cmd = format!(
                                        "curl -L -H 'Referer: {}' -H 'User-Agent: {}' {} -O --progress-bar",
                                        api.referer, api.user_agent, url
                                    );

                                    let mut player_cmd =
                                        env::var("SHIO_PLAYER_CMD").unwrap_or(default_cmd);

                                    if player_cmd.contains("{url}") {
                                        player_cmd = player_cmd.replace("{url}", url);
                                    }
                                    if player_cmd.contains("{referer}") {
                                        player_cmd = player_cmd.replace("{referer}", &api.referer)
                                    }
                                    if player_cmd.contains("{user_agent}") {
                                        player_cmd =
                                            player_cmd.replace("{user_agent}", &api.user_agent)
                                    }

                                    // windows
                                    #[cfg(not(unix))]
                                    let (shell, flag) = ("cmd", "/C");

                                    #[cfg(unix)]
                                    let (shell, flag) = ("sh", "-c");

                                    let cmd = Command::new(shell)
                                        .arg(flag)
                                        .arg(player_cmd)
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

        if let Some(resp) = &self.resp.search {
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
        };

        if let Some((_, resp, _)) = &self.resp.episode_list {
            let mut matches_result: Vec<(usize, u32)> = resp
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
        };

        if let Some((_, resp)) = &self.resp.episode_provider_list {
            let mut matches_result: Vec<(usize, u32)> = resp
                .iter()
                .enumerate()
                .filter_map(|(og_index, item)| {
                    let haystack = Utf32Str::new(&item.0, &mut buf);
                    pattern
                        .score(haystack, &mut self.matcher)
                        .map(|score| (og_index, score))
                })
                .collect();
            matches_result.sort_by(|a, b| b.1.cmp(&a.1));
            self.rows_to_data_index = matches_result.into_iter().map(|(i, _)| i).collect();
        }
    }

    /// render the skeleton before data is there
    fn render_skeleton(&self, frame: &mut Frame) {
        frame.render_widget(
            Paragraph::new(ASCII_ART)
                .centered()
                .block(Block::bordered().border_type(BorderType::Rounded)),
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
                    .style(Style::new().red()),
            ),
            area,
        );

        let cursor_x = area.x + 1 + (self.input.visual_cursor().max(scroll) - scroll) as u16;
        let cursor_y = area.y + 1;

        frame.set_cursor_position((cursor_x, cursor_y));
    }

    fn render_search_result(&mut self, frame: &mut Frame, area: Rect) {
        let Some(data) = &self.resp.search else {
            return;
        };

        let mut rows = vec![];
        for index in &self.rows_to_data_index {
            let item = &data[*index];

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
                    item.typename.clone(),
                    ep_count,
                ])
                .height(2),
            );
        }

        frame.render_stateful_widget(
            Table::new(
                rows,
                [
                    Constraint::Percentage(90),
                    Constraint::Percentage(5),
                    Constraint::Fill(1),
                ],
            )
            .style(Style::new().fg(Color::Cyan))
            .block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .title("Results")
                    .title_alignment(HorizontalAlignment::Center),
            )
            .highlight_symbol(self.select_icon.to_string())
            .row_highlight_style(Style::new().bg(Color::Cyan).fg(Color::Black)),
            area,
            &mut self.table_state,
        );
    }

    fn render_episode_list(&mut self, frame: &mut Frame, area: Rect, data: Vec<String>) {
        let mut rows = Vec::new();
        for index in &self.rows_to_data_index {
            let item = &data[*index];
            rows.push(Row::new(vec![item.clone()]).height(2));
        }

        frame.render_stateful_widget(
            Table::new(rows, [Constraint::Fill(1)])
                .highlight_symbol(self.select_icon.to_string())
                .row_highlight_style(Style::new().bg(Color::LightCyan).fg(Color::Black)),
            area,
            &mut self.table_state,
        );
    }

    fn render_episode_providers(
        &mut self,
        frame: &mut Frame,
        area: Rect,
        data: Vec<(String, String)>,
    ) {
        let mut rows = Vec::new();
        for index in &self.rows_to_data_index {
            let (provider_name, _link) = &data[*index];
            rows.push(Row::new(vec![provider_name.clone()]).height(4));
        }

        frame.render_stateful_widget(
            Table::new(rows, [Constraint::Percentage(10), Constraint::Fill(1)])
                .highlight_symbol(self.select_icon.to_string())
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
                if let Some(_) = &self.resp.search {
                    self.render_search_input(frame, top);
                    self.render_search_result(frame, bottom_left);
                    self.render_side_menu(frame, bottom_right);
                }
            }
            View::Episode => {
                if let Some((_, ep_list, _)) = &self.resp.episode_list {
                    self.render_search_input(frame, top);
                    self.render_episode_list(frame, bottom_left, ep_list.clone());
                    self.render_side_menu(frame, bottom_right);
                }
            }
            View::Provider => {
                if let Some((_, links)) = &self.resp.episode_provider_list {
                    self.render_search_input(frame, top);
                    self.render_episode_providers(frame, bottom_left, links.clone());
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
