use clap::Parser;
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::{
        self,
        // event::{self, Event},
    },
    layout::HorizontalAlignment,
    style::Style,
    widgets::{Block, BorderType},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, time::Duration};
use ureq::{self, Agent};

#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    name: String,
}

// Input Structures
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchInput {
    allow_adult: bool,
    allow_unknown: bool,
    query: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct QueryVariables {
    search: SearchInput,
    limit: i32,
    page: i32,
    translation_type: String,
    country_origin: String,
}

// Output Structures
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct AnimeEdge {
    #[serde(rename = "_id")]
    id: String,
    name: String,
    available_episodes: HashMap<String, Value>,
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
struct ApiResponse {
    data: DataWrapper,
}

fn search_anime(agent: &Agent, query: &str) -> Result<(), Box<dyn std::error::Error>> {
    let search_gql = r#"
    query( $search: SearchInput $limit: Int $page: Int $translationType: VaildTranslationTypeEnumType $countryOrigin: VaildCountryOriginEnumType ) {
        shows( search: $search limit: $limit page: $page translationType: $translationType countryOrigin: $countryOrigin ) {
            edges {
                _id
                name
                availableEpisodes
            }
        }
    }
    "#;

    let base_api = "https://api.allanime.day/api";
    let referer = "https://allmanga.to";

    let variables = QueryVariables {
        search: SearchInput {
            allow_adult: false,
            allow_unknown: false,
            query: query.to_string(),
        },
        limit: 40,
        page: 1,
        translation_type: "sub".to_string(),
        country_origin: "ALL".to_string(),
    };
    let variables_json = serde_json::to_string(&variables)?;

    let response = agent
        .get(base_api)
        .header("Referer", referer)
        .query("variables", &variables_json)
        .query("query", search_gql)
        .call()?;

    let parsed: ApiResponse = response.into_body().read_json()?;

    println!("Found {} results:", parsed.data.shows.edges.len());

    for anime in parsed.data.shows.edges {
        // Calculate episode count
        let ep_count = anime
            .available_episodes
            .get("sub")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        println!(
            "ID: {}\tName: {} ({} episodes)",
            anime.id, anime.name, ep_count
        );
    }

    Ok(())
}

fn main() -> color_eyre::eyre::Result<()> {
    color_eyre::install()?;
    let args = Args::parse();

    let config = Agent::config_builder()
        .timeout_per_call(Some(Duration::from_secs(12)))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Gecko/20100101 Firefox/121.0")
        .https_only(true)
        .build();

    let client = Agent::new_with_config(config);

    if let Err(e) = search_anime(&client, &args.name) {
        eprintln!("Error: {}", e);
    }
    ratatui::run(app)?;
    Ok(())
}

fn app(terminal: &mut DefaultTerminal) -> std::io::Result<()> {
    loop {
        terminal.draw(render)?;
        // if let Event::Key(key) = event::read()? {
        //     match key.code {
        //         event::KeyCode::Esc => {
        //             break Ok(());
        //         }
        //         _ => (),
        //     }
        // }
        if crossterm::event::read()?.is_key_press() {
            break Ok(());
        }
    }
}

fn render(frame: &mut Frame) {
    let outer_area = frame.area();

    let outer_block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().red())
        .title("shio")
        .title_alignment(HorizontalAlignment::Center);

    frame.render_widget(outer_block, outer_area);
}
