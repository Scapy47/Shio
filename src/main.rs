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
    #[arg()]
    /// Name of the anime to search
    name: String,
    #[arg(long, default_value_t = false)]
    debug: bool,
    // #[arg(long)]
    // user_agent: String,
}

//  NOTE: Response from search_anime()
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct AnimeEdge {
    #[serde(rename = "_id")]
    id: String,
    name: String,
    available_episodes: HashMap<String, Value>,
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

#[derive(Debug)]
struct Api {
    base_api: String,
    referer: String,
    agent: Agent,
}

impl Api {
    fn new() -> Self {
        let config = Agent::config_builder()
            .timeout_per_call(Some(Duration::from_secs(12)))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Gecko/20100101 Firefox/121.0")
            .https_only(true)
            .build();
        let agent = Agent::new_with_config(config);

        Api {
            base_api: "https://api.allanime.day/api".to_string(),
            referer: "https://allmanga.to".to_string(),
            agent: agent,
        }
    }

    fn request_api(&self, variables: String, gql: String) -> RequestBuilder<WithoutBody> {
        self.agent
            .get(&self.base_api)
            .header("Referer", &self.referer)
            .query("variables", variables)
            .query("query", gql)
    }

    fn search_anime(
        &self,
        query: String,
    ) -> Result<SearchResponse, Box<dyn std::error::Error + Send + Sync>> {
        let gql = "query( $search: SearchInput $limit: Int $page: Int $translationType: VaildTranslationTypeEnumType $countryOrigin: VaildCountryOriginEnumType ) { shows( search: $search limit: $limit page: $page translationType: $translationType countryOrigin: $countryOrigin ) { edges { _id name availableEpisodes __typename } }}".to_string();

        let variables_json = format!(
            r#"{{"search":{{"allowAdult":false,"allowUnknown":false,"query":"{}"}},"limit":40,"page":1,"translationType":"{}","countryOrigin":"ALL"}}"#,
            query,
            "sub" // TODO:
        );

        let resp = self.request_api(variables_json, gql).call()?;
        let parsed: SearchResponse = resp.into_body().read_json()?;

        Ok(parsed)
    }

    fn get_episode_links(
        &self,
        id: &str,
        ep: &str,
        debug: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let gql = "query ($showId: String!, $translationType: VaildTranslationTypeEnumType!, $episodeString: String!) { episode( showId: $showId translationType: $translationType episodeString: $episodeString ) { episodeString sourceUrls }}".to_string();

        let variables_json = format!(
            r#"{{"showId":"{}","translationType":"{}","episodeString":"{}"}}"#,
            id,
            "sub", //TODO:
            ep
        );
        let resp = self.request_api(variables_json, gql).call()?;
        let parsed: EpisodeResponse = resp.into_body().read_json()?;

        for source in parsed.data.episode.source_urls {
            let provider_name = &source.source_name;
            let encrypted_url = &source.source_url;

            let uri = if encrypted_url.starts_with("--") {
                &decrypt_url(&encrypted_url[2..])
            } else if encrypted_url.starts_with("//") {
                &format!("http:{}", &encrypted_url)
            } else {
                encrypted_url
            };

            let uri = if uri.contains("/clock") && !uri.contains("/clock.json") {
                &uri.replace("/clock", "/clock.json")
            } else {
                uri
            };

            let uri = if uri.starts_with("/apivtwo/") {
                &format!("https://allanime.day{}", uri)
            } else {
                uri
            };

            if debug {
                println!("\n--- Found Provider: {} ---", provider_name);
                println!("\tepisode:  {}", parsed.data.episode.episode_string);
                println!("\turi:      {}", uri);
                println!("\traw-uri:  {}", encrypted_url)
            }
        }

        Ok(())
    }

    //  INFO:
    fn get_episode_list(&self, id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let gql =
            "query ($showId: String!) { show( _id: $showId ) { _id availableEpisodesDetail }}"
                .to_string();
        let variables_json = format!(r#"{{"showId":"{}"}}"#, id);

        let resp = self.request_api(variables_json, gql).call()?;
        // let parsed: EpisodeResponse = resp.into_body().read_json()?;
        let res_str = resp.into_body().read_to_string()?;
        println!("\n{}", res_str);

        Ok(())
    }
}

#[derive(Debug)]
enum ApiResponse {
    Error(String),
    SearchResponse(Vec<AnimeEdge>),
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
    search_resp: Option<Vec<AnimeEdge>>,
    exit: bool,
    matcher: Matcher,
    search_index: Vec<usize>,
    widget_states: WidgetStates,
}

impl App {
    fn new() -> Self {
        let args = Args::parse();
        let api = Arc::new(Api::new());

        Self {
            widget_states: WidgetStates::default(),
            args: args,
            input: Input::default(),
            api: api,
            matcher: Matcher::new(Config::DEFAULT),
            search_index: vec![0],
            search_resp: None,
            exit: false,
        }
    }

    fn main_loop(&mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        let (tx, rx) = mpsc::channel::<ApiResponse>();

        let api_clone = self.api.clone();
        let name = self.args.name.clone();
        let tx_init = tx.clone();
        thread::spawn(move || match api_clone.search_anime(name) {
            Ok(resp) => tx_init.send(ApiResponse::SearchResponse(resp.data.shows.edges)),
            Err(e) => tx_init.send(ApiResponse::Error(e.to_string())),
        });

        while !self.exit {
            if let Ok(api_resp) = rx.try_recv() {
                match api_resp {
                    ApiResponse::SearchResponse(shows) => {
                        self.search_resp = Some(shows);

                        let count = self.search_resp.as_ref().unwrap().len();
                        self.search_index = (0..count).collect();
                    }
                    ApiResponse::Error(err_msg) => {
                        panic!("API Error: {}", err_msg);
                    }
                }
            }

            terminal.draw(|frame| self.render(frame))?;

            if event::poll(Duration::from_millis(16))? {
                let event = event::read()?;
                if let Event::Key(key) = event {
                    match key.code {
                        event::KeyCode::Esc => return Ok(()),
                        event::KeyCode::Down => self.widget_states.table.select_next(),
                        event::KeyCode::Up => self.widget_states.table.select_previous(),
                        event::KeyCode::Enter => {
                            // let Some(search_resp_vec) = &self.search_resp else {
                            //     return Ok(());
                            // };
                            //
                            // let table_column = self.widget_states.table.selected().unwrap_or(0);
                            // let id = search_resp_vec[self.search_index[table_column]].id.clone();

                            // let api_clone = self.api.clone()
                            // thread::spawn(move || tx.send(api_clone.get_episode_list(&id)).unwrap() );
                        }
                        _ => {
                            self.input.handle_event(&event);
                            self.update_search_index();
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn update_search_index(&mut self) {
        let Some(search_resp_vec) = &self.search_resp else {
            return;
        };

        let pattern = Pattern::new(
            &self.input.value(),
            CaseMatching::Smart,
            Normalization::Smart,
            AtomKind::Fuzzy,
        );

        let mut buf = Vec::new();
        let mut matches_result: Vec<(usize, u32)> = search_resp_vec
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

        self.search_index = matches_result.into_iter().map(|(i, _)| i).collect();
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
        let Some(search_resp_vec) = &self.search_resp else {
            return;
        };

        let mut rows = vec![];
        for index in &self.search_index {
            let item = &search_resp_vec[*index];

            let ep_count = item
                .available_episodes
                .get("sub")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);

            rows.push(Row::new(vec![
                item.name.clone(),
                item.typename.clone(),
                ep_count.to_string(),
                item.id.clone(),
            ]));
        }

        frame.render_stateful_widget(
            Table::new(
                rows,
                [
                    Constraint::Percentage(60),
                    Constraint::Percentage(10),
                    Constraint::Percentage(10),
                    Constraint::Percentage(20),
                ],
            )
            .block(
                Block::bordered()
                    .border_type(BorderType::Rounded)
                    .title("Results")
                    .title_alignment(HorizontalAlignment::Center),
            )
            .row_highlight_style(Style::new().bg(Color::LightBlue).fg(Color::Black)),
            area,
            &mut self.widget_states.table,
        );
    }

    // fn render_episode_list(&mut self, frame: &mut Frame, area: Rect) {}
    fn render_side_menu(&mut self, frame: &mut Frame, area: Rect) {
        frame.render_widget(Paragraph::new(ASCII_ART).centered(), area);
    }

    fn render(&mut self, frame: &mut Frame) {
        let [top, bottom] =
            Layout::vertical([Constraint::Percentage(10), Constraint::Fill(1)]).areas(frame.area());

        let [bottom_left, bottom_right] =
            Layout::horizontal([Constraint::Percentage(70), Constraint::Fill(1)]).areas(bottom);

        self.render_search_input(frame, top);
        self.render_search_result(frame, bottom_left);
        self.render_side_menu(frame, bottom_right);
    }
}

fn main() -> color_eyre::eyre::Result<()> {
    color_eyre::install()?;

    let args = Args::parse();
    let api = Api::new();

    if let Err(e) = api.get_episode_links("HM5zSCbGwSAsWPFjX", "1", args.debug) {
        eprintln!("error: {}", e);
    }
    if let Err(e) = api.get_episode_list("HM5zSCbGwSAsWPFjX") {
        eprintln!("error: {}", e);
    }

    ratatui::run(|terminal| App::new().main_loop(terminal))?;
    Ok(())
}
