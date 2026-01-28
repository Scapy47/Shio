use clap::Parser;
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event},
    layout::{Constraint, Layout},
    style::Style,
    widgets::{Block, BorderType, Paragraph},
};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::HashMap, time::Duration};
use tui_input::{Input, backend::crossterm::EventHandler};
use ureq::{Agent, RequestBuilder, typestate::WithoutBody};

mod utils;
use crate::utils::{LOGO, decrypt_url};

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

//  NOTE: Response from search_anime
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

//  NOTE: Response for get_episode_links
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

    fn search_anime(&self, query: &str) -> Result<SearchResponse, Box<dyn std::error::Error>> {
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
struct App {
    args: Args,
    input: Input,
    exit: bool,
}
impl App {
    fn new() -> Self {
        App {
            args: Args::parse(),
            input: Input::default(),
            exit: false,
        }
    }

    fn main_loop(&mut self, terminal: &mut DefaultTerminal) -> std::io::Result<()> {
        while !self.exit {
            terminal.draw(|frame| self.render(frame))?;
            // if crossterm::event::read()?.is_key_press() {
            //     break;
            // }
            let event = event::read()?;
            if let Event::Key(key) = event {
                match key.code {
                    event::KeyCode::Esc => return Ok(()),
                    _ => {
                        self.input.handle_event(&event);
                    }
                }
            }
        }
        Ok(())
    }

    fn render(&self, frame: &mut Frame) {
        let api = Api::new();

        let [top, bottom] =
            Layout::vertical([Constraint::Percentage(10), Constraint::Fill(1)]).areas(frame.area());

        let [bottom_left, bottom_right] =
            Layout::horizontal([Constraint::Percentage(70), Constraint::Fill(1)]).areas(bottom);

        let input = Paragraph::new(self.input.value()).block(
            Block::bordered()
                .title("Input")
                .border_type(BorderType::Rounded)
                .style(Style::new().green()),
        );
        frame.render_widget(input, top);

        let block_left = Block::bordered()
            .title("")
            .border_type(BorderType::Rounded)
            .style(Style::new().red());
        frame.render_widget(block_left, bottom_left);

        let block_right = Block::bordered()
            .border_type(BorderType::Rounded)
            .style(Style::new().magenta());
        frame.render_widget(&block_right, bottom_right);

        let [bottom_right_top, bottom_right_bottom] =
            Layout::vertical([Constraint::Percentage(50), Constraint::Fill(1)]).areas(bottom_right);

        frame.render_widget(
            Block::bordered()
                .border_type(BorderType::Rounded)
                .style(Style::new().blue()),
            block_right.inner(bottom_right_bottom),
        );
        frame.render_widget(
            Paragraph::new(LOGO).centered(),
            block_right.inner(bottom_right_top),
        );

        // frame.render_widget(
        //     Paragraph::new("Hello firend").block(Block::bordered()),
        //     frame.area(),
        // );

        // let mut row = vec![];
        // let resp = api.search_anime(&self.args.name).unwrap();
        // for anime in resp.data.shows.edges {
        //     let ep_count = anime
        //         .available_episodes
        //         .get("sub")
        //         .and_then(|v| v.as_u64())
        //         .unwrap_or(0);
        //
        //     row.push(Row::new(vec![
        //         anime.name,
        //         anime.typename,
        //         ep_count.to_string(),
        //         anime.id,
        //     ]));
        // }
        //
        // row.insert(
        //     0,
        //     Row::new(vec!["Name", "Type", "Episodes", "ID"]).style(Style::new().red()),
        // );
        //
        // let outer_area = frame.area();
        // let outer_block = Block::bordered()
        //     .border_type(BorderType::Rounded)
        //     .border_style(Style::new().red())
        //     .title("shio")
        //     .title_alignment(HorizontalAlignment::Center);
        //
        // let inner_area = outer_block.inner(outer_area);
        // let layout_inner = Layout::default()
        //     .direction(Direction::Horizontal)
        //     .constraints(vec![Constraint::Percentage(70), Constraint::Percentage(30)])
        //     .split(inner_area);
        //
        // frame.render_widget(outer_block, outer_area);
        //
        // frame.render_widget(
        //     Table::new(
        //         row,
        //         [
        //             Constraint::Percentage(70),
        //             Constraint::Percentage(5),
        //             Constraint::Percentage(10),
        //             Constraint::Percentage(15),
        //         ],
        //     )
        //     .block(Block::bordered()),
        //     layout_inner[0],
        // );

        //     frame.render_widget(
        //         Paragraph::new("test").block(Block::bordered()),
        //         layout_inner[1],
        //     );
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
