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
    text::{Line, Span},
    widgets::{Block, BorderType, Cell, Paragraph, Row, Table, TableState},
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
    utils::decrypt_url,
};

#[derive(Parser, Debug)]
struct Args {
    /// Name of the anime to watch
    #[arg()]
    name: Option<String>,

    /// Audio mode to use
    #[arg(short, long, value_enum, default_value_t = Mode::Sub)]
    mode: Mode,

    /// Enable debug output
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
    /// select icon
    select_icon: String,
    /// current view that is being rendered
    view: View,
    /// condition that is allowing loop to contine
    exit: bool,
    /// arguments passed to cli
    args: Args,
    /// search bar input state
    input: Input,
    api: Arc<Api>,
    resp: Resp,
    matcher: Matcher,
    rows_to_data_index: Vec<usize>,
    table_state: TableState,
    ui_loop_tick: Instant,
}

impl App {
    fn new() -> Self {
        let args = Args::parse();
        let mode = args.mode;
        let api = Arc::new(Api::new(mode, args.debug));

        Self {
            select_icon: String::default(),
            table_state: TableState::default(),
            args,
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
        thread::spawn(
            move || match api_clone.search_anime(name.unwrap_or_default().as_str()) {
                Ok(resp) => tx_clone.send(Some(Resp {
                    search: Some(resp.data.shows.edges),
                    ..Default::default()
                })),
                Err(e) => {
                    eprintln!("Error getting search results: {}", e);
                    tx_clone.send(None)
                }
            },
        );

        while !self.exit {
            if let Ok(Some(resp)) = rx.try_recv() {
                if let Some(search_resp) = resp.search {
                    self.rows_to_data_index = (0..search_resp.len()).collect();
                    self.resp.search = Some(search_resp);
                    self.view = View::Search
                }
                if let Some(ep_list_resp) = resp.episode_list {
                    self.rows_to_data_index = (0..ep_list_resp.1.len()).collect();
                    self.resp.episode_list = Some(ep_list_resp);
                    self.view = View::Episode
                }
                if let Some(ep_provider_list_resp) = resp.episode_provider_list {
                    self.rows_to_data_index = (0..ep_provider_list_resp.1.len()).collect();
                    self.resp.episode_provider_list = Some(ep_provider_list_resp);
                    self.view = View::Provider
                }

                self.table_state.select(Some(0));
                self.input.reset();
            }

            // match self.view {
            //     View::Loading => (),
            //     View::Search => {}
            //     View::Episode => {}
            //     View::Provider => {}
            // }

            terminal.draw(|frame| self.render(frame))?;

            self.select_icon_animation();

            if event::poll(Duration::from_millis(16))? {
                let event = event::read()?;
                if let Event::Key(key) = event {
                    match key.code {
                        event::KeyCode::Esc => return Ok(()),
                        event::KeyCode::Char('q')
                            if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                        {
                            return Ok(());
                        }
                        event::KeyCode::Down => self.table_state.select_next(),
                        event::KeyCode::Char('j') | event::KeyCode::Char('n')
                            if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                        {
                            self.table_state.select_next()
                        }
                        event::KeyCode::Up => self.table_state.select_previous(),
                        event::KeyCode::Char('k') | event::KeyCode::Char('p')
                            if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                        {
                            self.table_state.select_previous()
                        }
                        event::KeyCode::Backspace | event::KeyCode::Char('h')
                            if key.modifiers.contains(event::KeyModifiers::CONTROL) =>
                        {
                            match self.view {
                                View::Loading => (),
                                View::Search => return Ok(()),
                                View::Episode => self.view = View::Search,
                                View::Provider => self.view = View::Episode,
                            }
                            self.input.reset();
                            self.table_state.select(Some(0));
                            self.update_row_to_data_index()
                        }
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
                                            eprintln!("Error getting episode list: {}", e);
                                            tx_clone.send(None)
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
                                                eprintln!("Error getting episode links: {}", e);
                                                tx_clone.send(None)
                                            }
                                        }
                                    });
                                }
                            }
                            View::Provider => {
                                if let Some((_, links)) = &self.resp.episode_provider_list {
                                    let Some(row) = self.table_state.selected() else {
                                        return Ok(());
                                    };
                                    let (_provider, url) = &links[self.rows_to_data_index[row]];
                                    let api = self.api.clone();

                                    let url = if url.contains("clock.json")
                                        || url.contains("https://allanime.day")
                                    {
                                        api.resolve_clock_urls(url).unwrap()
                                    } else {
                                        url.to_string()
                                    };

                                    let mut player_cmd =
                                        env::var("SHIO_PLAYER_CMD").unwrap_or(
                                        "curl -L -H 'Referer: {referer}' -H 'User-Agent: {user_agent}' {url} -O --progress-bar".to_string());

                                    if player_cmd.contains("{url}") {
                                        player_cmd = player_cmd.replace("{url}", &url);
                                    }
                                    if player_cmd.contains("{referer}") {
                                        player_cmd = player_cmd.replace("{referer}", api.referer)
                                    }
                                    if player_cmd.contains("{user_agent}") {
                                        player_cmd =
                                            player_cmd.replace("{user_agent}", api.user_agent)
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

                                    if cmd == 1 {
                                        self.exit = true;
                                    }
                                    self.view = View::Episode
                                }
                            }
                        },
                        _ => {
                            self.input.handle_event(&event);
                            self.update_row_to_data_index();
                            self.table_state.select(Some(0));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn fuzzy_reorder(&mut self, str_vec: Vec<String>, buff: &mut Vec<char>) {
        let pattern = Pattern::new(
            self.input.value(),
            CaseMatching::Smart,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );

        let mut vec: Vec<(usize, u32)> = Vec::new();
        for (i, e_str) in str_vec.iter().enumerate() {
            if let Some(score) = pattern.score(Utf32Str::new(e_str, buff), &mut self.matcher) {
                vec.push((i, score));
            };
        }

        vec.sort_by(|a, b| b.1.cmp(&a.1));
        self.rows_to_data_index = vec.into_iter().map(|(i, _)| i).collect()
    }

    /// update the index of rows to data pointer vec
    fn update_row_to_data_index(&mut self) {
        let mut buf = Vec::new();

        match self.view {
            View::Loading => (),
            View::Search => {
                if let Some(resp) = &self.resp.search {
                    self.fuzzy_reorder(
                        resp.iter()
                            .map(|item| {
                                if let Some(english_name) = &item.english_name {
                                    format!("{} {}", item.name, english_name)
                                } else {
                                    item.name.to_string()
                                }
                            })
                            .collect(),
                        &mut buf,
                    )
                }
            }

            View::Episode => {
                if let Some((_, resp, _)) = &self.resp.episode_list {
                    self.fuzzy_reorder(resp.iter().map(|item| item.to_string()).collect(), &mut buf)
                }
            }

            View::Provider => {
                if let Some((_, resp)) = &self.resp.episode_provider_list {
                    self.fuzzy_reorder(
                        resp.iter().map(|item| item.0.to_string()).collect(),
                        &mut buf,
                    )
                }
            }
        }
    }

    fn render_search_input(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(
            Paragraph::new(self.input.value())
                .alignment(HorizontalAlignment::Center)
                .block(
                    Block::bordered()
                        .title("Fuzzy Search")
                        .title_style(Style::new().green().bold())
                        .style(Style::new().red())
                        .border_type(BorderType::Rounded),
                ),
            area,
        );
    }

    /// render the skeleton before data is there
    fn render_skeleton(&self, frame: &mut Frame, area: Rect) {
        frame.render_widget(
            Paragraph::new(include_str!("../assets/art.txt"))
                .style(Style::default().bold().cyan())
                .centered()
                .block(
                    Block::bordered()
                        .style(Style::new().red())
                        .border_type(BorderType::Rounded),
                ),
            area,
        );
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
                    Cell::from(
                        Line::styled((index + 1).to_string(), Style::default().yellow().bold())
                            .alignment(HorizontalAlignment::Center),
                    ),
                    Cell::from(vec![
                        Line::from(Span::styled(
                            item.name.as_str(),
                            Style::new().magenta().bold(),
                        )),
                        Line::from(Span::styled(
                            if english_name != item.name {
                                english_name
                            } else {
                                ""
                            },
                            Style::new().red().bold(),
                        )),
                    ]),
                    Cell::from(
                        Line::styled(ep_count, Style::new().bold())
                            .alignment(HorizontalAlignment::Center),
                    ),
                ])
                .height(3),
            )
        }

        let header = Row::new(vec![
            Line::from("#").alignment(HorizontalAlignment::Center),
            Line::from("Name").alignment(HorizontalAlignment::Center),
            Line::from("Episodes").alignment(HorizontalAlignment::Center),
        ])
        .style(Style::default().bold().yellow())
        .bottom_margin(1);

        frame.render_stateful_widget(
            Table::new(
                rows,
                [
                    Constraint::Percentage(5),
                    Constraint::Percentage(85),
                    Constraint::Fill(1),
                ],
            )
            .header(header)
            .style(Style::new().fg(Color::Cyan))
            .highlight_symbol(self.select_icon.to_string())
            .row_highlight_style(Style::new().bg(Color::Cyan).fg(Color::Black))
            .block(Block::bordered().border_type(BorderType::Rounded)),
            area,
            &mut self.table_state,
        );
    }

    fn render_episode_list(&mut self, frame: &mut Frame, area: Rect) {
        let Some((_, ep_list, _)) = &self.resp.episode_list else {
            return;
        };

        let mut rows = Vec::new();
        for index in &self.rows_to_data_index {
            let item = ep_list[*index].as_str();
            rows.push(
                Row::new(vec![
                    Line::styled(item, Style::new().red().bold())
                        .alignment(HorizontalAlignment::Center),
                ])
                .height(2),
            )
        }

        let header = Row::new(vec![
            Line::from("Episodes").alignment(HorizontalAlignment::Center),
        ])
        .style(Style::default().bold().yellow())
        .bottom_margin(1);

        frame.render_stateful_widget(
            Table::new(rows, [Constraint::Fill(1)])
                .header(header)
                .style(Style::new().fg(Color::Cyan))
                .highlight_symbol(self.select_icon.to_string())
                .row_highlight_style(Style::new().bg(Color::LightCyan).fg(Color::Black))
                .block(Block::bordered().border_type(BorderType::Rounded)),
            area,
            &mut self.table_state,
        );
    }

    fn render_episode_providers(&mut self, frame: &mut Frame, area: Rect) {
        let Some((_, links_list)) = &self.resp.episode_provider_list else {
            return;
        };

        let mut rows = Vec::new();
        for index in &self.rows_to_data_index {
            let (provider_name, _link) = &links_list[*index];
            rows.push(
                Row::new(vec![
                    Line::styled(provider_name, Style::new().red().bold())
                        .alignment(HorizontalAlignment::Center),
                ])
                .height(2),
            );
        }

        let header = Row::new(vec![
            Line::from("Provider").alignment(HorizontalAlignment::Center),
        ])
        .style(Style::default().bold().yellow())
        .bottom_margin(1);

        frame.render_stateful_widget(
            Table::new(rows, [Constraint::Fill(1)])
                .header(header)
                .style(Style::new().fg(Color::Cyan))
                .highlight_symbol(self.select_icon.to_string())
                .row_highlight_style(Style::new().bg(Color::LightCyan).fg(Color::Black))
                .block(Block::bordered().border_type(BorderType::Rounded)),
            area,
            &mut self.table_state,
        );
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect, line: Line) {
        frame.render_widget(
            Paragraph::new(line)
                .alignment(HorizontalAlignment::Center)
                .block(
                    Block::bordered()
                        .border_type(BorderType::Rounded)
                        .style(Style::new().red()),
                ),
            area,
        );
    }

    fn render(&mut self, frame: &mut Frame) {
        let [top, middle, bottom] = Layout::vertical([
            Constraint::Length(3),
            Constraint::Fill(1),
            Constraint::Length(3),
        ])
        .areas(frame.area());

        self.render_search_input(frame, top);

        match self.view {
            View::Loading => self.render_skeleton(frame, middle),
            View::Search => {
                if self.resp.search.is_some() {
                    self.render_search_result(frame, middle);
                }
            }
            View::Episode => {
                if self.resp.episode_list.is_some() {
                    self.render_episode_list(frame, middle);
                }
            }
            View::Provider => {
                if self.resp.episode_provider_list.is_some() {
                    self.render_episode_providers(frame, middle);
                }
            }
        }

        self.render_footer(
            frame,
            bottom,
            Line::from(vec![
                Span::raw("move "),
                Span::styled("Up/Down ", Style::default().bold().yellow()),
                Span::raw("using "),
                Span::styled("↑ / ctrl+k / ctrl+p ", Style::default().bold().yellow()),
                Span::raw("and "),
                Span::styled("↓ / ctrl+j / ctrl+n ", Style::default().bold().yellow()),
                Span::raw("keys, "),
                Span::raw("press "),
                Span::styled("Enter ", Style::default().bold().green()),
                Span::raw("to "),
                Span::styled("Select ", Style::default().bold().green()),
                Span::raw("and "),
                Span::styled("ctrl+<BS> ", Style::default().bold().cyan()),
                Span::raw("to go "),
                Span::styled("Back", Style::default().bold().cyan()),
            ]),
        );
    }
}

fn main() -> color_eyre::eyre::Result<()> {
    color_eyre::install()?;

    let mut app = App::new();
    ratatui::run(|terminal| app.main_loop(terminal))?;
    Ok(())
}
