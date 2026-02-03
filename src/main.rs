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
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::HashMap,
    process::Command,
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};
use tui_input::{Input, backend::crossterm::EventHandler};
use ureq::{Agent, RequestBuilder, typestate::WithoutBody};

mod utils;
use crate::utils::{ASCII_ART, decrypt_url};

#[derive(Parser, Debug)]
struct Args {
    /// Name of the anime to search
    name: String,

    /// Print debuging info
    #[arg(long)]
    debug: bool,
}

//  NOTE: Response from search_anime()
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct AnimeEdge {
    #[serde(rename = "_id")]
    id: String,
    name: String,

    english_name: Option<String>,
    available_episodes: Option<HashMap<String, Value>>,
    #[serde(rename = "__typename")]
    typename: String,
}

#[derive(Deserialize, Debug)]
struct ShowsData {
    edges: Vec<AnimeEdge>,
}

#[derive(Deserialize, Debug)]
struct DataWrapper {
    shows: ShowsData,
}

#[derive(Deserialize, Debug)]
struct SearchResponse {
    data: DataWrapper,
}

//  NOTE: Response for get_episode_links()
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct SourceUrl {
    source_url: String,
    source_name: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct EpisodeData {
    episode_string: String,
    source_urls: Vec<SourceUrl>,
}

#[derive(Deserialize, Debug)]
struct EpisodeDataWrapper {
    episode: EpisodeData,
}

#[derive(Deserialize, Debug)]
struct EpisodeResponse {
    data: EpisodeDataWrapper,
}

//  NOTE: Response for get_episode_list()
#[derive(Deserialize, Debug)]
struct ShowDetail {
    #[serde(rename = "_id")]
    id: String,
    name: String,
    #[serde(rename = "availableEpisodesDetail")]
    available_episodes_detail: HashMap<String, Vec<String>>,
}

#[derive(Deserialize, Debug)]
struct ShowDetailData {
    show: ShowDetail,
}

#[derive(Deserialize, Debug)]
struct EpisodeListResponse {
    data: ShowDetailData,
}

#[derive(Debug)]
struct Api {
    base_api: String,
    referer: String,
    agent: Agent,
    user_agent: String,
    mode: String,
    debug: bool,
}

#[derive(Debug)]
enum Mode {
    Sub,
    Dub,
    Raw,
}

impl Api {
    fn new(mode: Mode, debug: bool) -> Self {
        let user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Gecko/20100101 Firefox/121.0";
        let config = Agent::config_builder()
            .timeout_per_call(Some(Duration::from_secs(12)))
            .user_agent(user_agent)
            .https_only(true)
            .build();
        let agent = Agent::new_with_config(config);

        let mode = match mode {
            Mode::Sub => "sub",
            Mode::Dub => "dub",
            Mode::Raw => "raw",
        }
        .to_string();

        Api {
            base_api: "https://api.allanime.day/api".to_string(),
            referer: "https://allmanga.to".to_string(),
            agent: agent,
            user_agent: user_agent.to_string(),
            mode: mode,
            debug: debug,
        }
    }

    fn request_api(&self, variables: &str, gql: &str) -> RequestBuilder<WithoutBody> {
        self.agent
            .get(&self.base_api)
            .header("Referer", &self.referer)
            .query("variables", variables)
            .query("query", gql)
    }

    /// Search for anime with its name
    fn search_anime(
        &self,
        query: String,
    ) -> Result<SearchResponse, Box<dyn std::error::Error + Send + Sync>> {
        let gql = "query( $search: SearchInput $limit: Int $page: Int $translationType: VaildTranslationTypeEnumType $countryOrigin: VaildCountryOriginEnumType ) { shows( search: $search limit: $limit page: $page translationType: $translationType countryOrigin: $countryOrigin ) { edges { _id name englishName availableEpisodes __typename } }}";

        let variables_json = &format!(
            r#"{{"search":{{"allowAdult":false,"allowUnknown":false,"query":"{}"}},"limit":40,"page":1,"translationType":"{}","countryOrigin":"ALL"}}"#,
            query, self.mode
        );

        let resp = self.request_api(variables_json, gql).call()?;
        let parsed: SearchResponse = resp.into_body().read_json()?;

        Ok(parsed)
    }

    /// Get the links that can be played/download
    fn get_episode_links(
        &self,
        id: &str,
        ep: &str,
    ) -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
        let gql = "query ($showId: String!, $translationType: VaildTranslationTypeEnumType!, $episodeString: String!) { episode( showId: $showId translationType: $translationType episodeString: $episodeString ) { episodeString sourceUrls }}";

        let variables_json = &format!(
            r#"{{"showId":"{}","translationType":"{}","episodeString":"{}"}}"#,
            id, self.mode, ep
        );
        let resp = self.request_api(variables_json, gql).call()?;
        let parsed: EpisodeResponse = resp.into_body().read_json()?;

        let mut vec = Vec::new();
        for source in parsed.data.episode.source_urls {
            let provider_name = source.source_name;
            let raw_uri = source.source_url;

            let uri = if raw_uri.starts_with("--") {
                decrypt_url(&&raw_uri[2..])
            } else if raw_uri.starts_with("//") {
                format!("http:{}", raw_uri)
            } else {
                raw_uri.clone()
            };

            let uri = if uri.contains("/clock") && !uri.contains("/clock.json") {
                uri.replace("/clock", "/clock.json")
            } else {
                uri
            };

            let uri = if uri.starts_with("/apivtwo/") {
                format!("https://allanime.day{}", uri)
            } else {
                uri
            };

            if self.debug {
                println!("-------------------------");
                println!("--- Found Provider: {} ---", &provider_name);
                println!("\tepisode:  {}", parsed.data.episode.episode_string);
                println!("\turi:      {}", &uri);
                println!("\traw-uri:  {}", raw_uri);
                println!("-------------------------");
            }

            vec.push((provider_name, uri));
        }

        Ok(vec)
    }

    /// Get list of episodes available from api
    fn get_episode_list(
        &self,
        id: &str,
    ) -> Result<(String, Vec<String>, String), Box<dyn std::error::Error>> {
        let gql =
            "query ($showId: String!) { show( _id: $showId ) { _id name availableEpisodesDetail }}";
        let variables_json = &format!(r#"{{"showId":"{}"}}"#, id);

        let resp = self.request_api(variables_json, gql).call()?;
        let parsed: EpisodeListResponse = resp.into_body().read_json()?;

        let mut episodes = parsed
            .data
            .show
            .available_episodes_detail
            .get(&self.mode)
            .ok_or(format!("No episodes found for mode '{}'", &self.mode))?
            .clone();

        episodes.sort_by(|a, b| {
            let a_num = a.parse::<f64>().unwrap_or(0.0);
            let b_num = b.parse::<f64>().unwrap_or(0.0);
            a_num
                .partial_cmp(&b_num)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        if self.debug {
            println!("-------------------------");
            println!("\tID:    {}", parsed.data.show.id);
            println!("\tEPISODES: {:?}", &episodes);
            println!("-------------------------");
        }

        Ok((parsed.data.show.name, episodes, parsed.data.show.id))
    }
}

#[derive(Debug)]
enum ApiResponse {
    Error(String),
    SearchResp(Vec<AnimeEdge>),
    EpisodeListResp((String, Vec<String>, String)),
    EpisodeLinksResp(Vec<(String, String)>),
}

#[derive(Debug, Default)]
struct WidgetStates {
    table: TableState,
}

#[derive(Debug)]
struct App {
    args: Args,
    input: Input,
    api: Arc<Api>,
    resp: ApiResponse,
    exit: bool,
    matcher: Matcher,
    rows_to_data_index: Vec<usize>,
    widget_states: WidgetStates,
}

impl App {
    fn new() -> Self {
        let args = Args::parse();
        let api = Arc::new(Api::new(Mode::Sub, args.debug));

        Self {
            widget_states: WidgetStates::default(),
            args: args,
            input: Input::default(),
            api: api,
            matcher: Matcher::new(Config::DEFAULT),
            rows_to_data_index: Vec::new(),
            resp: ApiResponse::Error(String::new()),
            exit: false,
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

        self.widget_states.table.select(Some(0));

        while !self.exit {
            if let Ok(api_resp) = rx.try_recv() {
                match &api_resp {
                    ApiResponse::SearchResp(resp) => {
                        self.rows_to_data_index = (0..resp.len()).collect();
                    }
                    ApiResponse::EpisodeListResp((_, ep_list, _)) => {
                        self.rows_to_data_index = (0..ep_list.len()).collect();
                    }
                    ApiResponse::EpisodeLinksResp(links) => {
                        self.rows_to_data_index = (0..links.len()).collect();
                    }
                    _ => {}
                }

                self.resp = api_resp;
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
                        event::KeyCode::Down => self.widget_states.table.select_next(),
                        event::KeyCode::Up => self.widget_states.table.select_previous(),
                        event::KeyCode::Left => self.widget_states.table.select_next_column(),
                        event::KeyCode::Right => self.widget_states.table.select_previous_column(),
                        event::KeyCode::Enter => match &self.resp {
                            ApiResponse::SearchResp(resp) => {
                                let Some(row) = self.widget_states.table.selected() else {
                                    return Ok(());
                                };
                                let id = resp[self.rows_to_data_index[row]].id.clone();

                                let tx_clone = tx.clone();
                                let api_clone = self.api.clone();
                                thread::spawn(move || match api_clone.get_episode_list(&id) {
                                    Ok(resp) => tx_clone.send(ApiResponse::EpisodeListResp(resp)),
                                    Err(e) => tx_clone.send(ApiResponse::Error(e.to_string())),
                                });
                            }
                            ApiResponse::EpisodeListResp((_, list, id)) => {
                                let Some(row) = self.widget_states.table.selected() else {
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
                                        Err(e) => tx_clone.send(ApiResponse::Error(e.to_string())),
                                    }
                                });
                            }
                            ApiResponse::EpisodeLinksResp(links) => {
                                let Some(row) = self.widget_states.table.selected() else {
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
                            ApiResponse::Error(e) => {
                                panic!("Error: {}", e)
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
            ApiResponse::EpisodeLinksResp(links) => {
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

    fn render_search_input(&mut self, frame: &mut Frame, area: Rect) {
        frame.render_widget(
            Paragraph::new(self.input.value()).block(
                Block::bordered()
                    .title("Search")
                    .title_style(Style::new().bold())
                    .title_alignment(HorizontalAlignment::Center)
                    .border_type(BorderType::Rounded)
                    .style(Style::new().green()),
            ),
            area,
        );
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
                .unwrap_or(0);

            let english_name = item.english_name.as_deref().unwrap_or(&item.name);

            rows.push(Row::new(vec![
                english_name.to_string(),
                item.name.clone(),
                ep_count.to_string(),
                item.id.clone(),
                item.typename.clone(),
            ]));
        }

        frame.render_stateful_widget(
            Table::new(
                rows,
                [
                    Constraint::Percentage(60),
                    Constraint::Percentage(20),
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
            .row_highlight_style(Style::new().bg(Color::LightBlue).fg(Color::Black)),
            area,
            &mut self.widget_states.table,
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
            &mut self.widget_states.table,
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
            &mut self.widget_states.table,
        );
    }

    fn render_side_menu(&mut self, frame: &mut Frame, area: Rect) {
        frame.render_widget(Paragraph::new(ASCII_ART).centered(), area);
    }

    fn render(&mut self, frame: &mut Frame) {
        let [top, bottom] =
            Layout::vertical([Constraint::Percentage(10), Constraint::Fill(1)]).areas(frame.area());

        let [bottom_left, bottom_right] =
            Layout::horizontal([Constraint::Percentage(70), Constraint::Fill(1)]).areas(bottom);

        self.render_search_input(frame, top);

        match &self.resp {
            ApiResponse::SearchResp(_) => {
                self.render_search_result(frame, bottom_left);
                self.render_side_menu(frame, bottom_right);
            }
            ApiResponse::EpisodeListResp((_, ep_list, _)) => {
                self.render_episode_list(frame, bottom_left, ep_list.clone());
                self.render_side_menu(frame, bottom_right);
            }
            ApiResponse::EpisodeLinksResp(links) => {
                self.render_episode_links(frame, bottom_left, links.clone());
            }
            ApiResponse::Error(_) => {
                return;
            }
        }
    }
}

fn main() -> color_eyre::eyre::Result<()> {
    color_eyre::install()?;

    ratatui::run(|terminal| App::new().main_loop(terminal))?;
    Ok(())
}
