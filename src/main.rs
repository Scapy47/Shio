use clap::Parser;
use ratatui::{
    DefaultTerminal, Frame,
    crossterm::{self},
    layout::{Constraint, Direction, HorizontalAlignment, Layout},
    style::Style,
    widgets::{Block, BorderType, Paragraph, Row, Table},
};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::HashMap, time::Duration};
use ureq::{self, Agent};

use crate::utils::decrypt_url;

mod utils;

#[derive(Parser, Debug)]
struct Args {
    #[arg()]
    name: String,
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

// 3. Structs for Response
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct SourceUrl {
    source_url: String,
    source_name: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct EpisodeData {
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

fn search_anime(agent: &Agent, query: &str) -> Result<ApiResponse, Box<dyn std::error::Error>> {
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

    let variables_json = format!(
        r#"{{"search":{{"allowAdult":false,"allowUnknown":false,"query":"{}"}},"limit":40,"page":1,"translationType":"{}","countryOrigin":"ALL"}}"#,
        query, "sub"
    );

    let response = agent
        .get(base_api)
        .header("Referer", referer)
        .query("variables", &variables_json)
        .query("query", search_gql)
        .call()?;

    let parsed: ApiResponse = response.into_body().read_json()?;

    Ok(parsed)
}

fn get_episode_links(agent: &Agent, id: &str, ep: &str) -> Result<(), Box<dyn std::error::Error>> {
    let episode_embed_gql = r#"
    query ($showId: String!, $translationType: VaildTranslationTypeEnumType!, $episodeString: String!) { episode( showId: $showId translationType: $translationType episodeString: $episodeString ) { episodeString sourceUrls }}
    "#;
    let base_api = "https://api.allanime.day/api";
    let referer = "https://allmanga.to";

    let response = agent
        .get(base_api)
        .header("Referer", referer)
        .query(
            "variables",
            format!(
                r#"{{"showId":"{}","translationType":"{}","episodeString":"{}"}}"#,
                id, "sub", ep
            ),
        )
        .query("query", &episode_embed_gql)
        .call()?;

    let parsed: EpisodeResponse = response.into_body().read_json()?;

    for source in parsed.data.episode.source_urls {
        let provider_name = &source.source_name;
        let encrypted_url = &source.source_url;

        let uri = if encrypted_url.starts_with("--") {
            &decrypt_url(&encrypted_url[2..])
        } else if encrypted_url.starts_with("//") {
            &format!("http:{}", &encrypted_url)
        } else {
            println!("raw (without decrypt_url): {}", &encrypted_url);
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

        println!("\n--- Found Provider: {} ---", provider_name);
        println!("uris:\t{}", uri);
    }

    Ok(())
}

fn main() -> color_eyre::eyre::Result<()> {
    color_eyre::install()?;
    // let args = Args::parse();
    //
    let config = Agent::config_builder()
        .timeout_per_call(Some(Duration::from_secs(12)))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Gecko/20100101 Firefox/121.0")
        .https_only(true)
        .build();

    let client = Agent::new_with_config(config);

    // let resp = search_anime(&client, &args.name).unwrap();

    // println!("Found {} results:", resp.data.shows.edges.len());
    //
    // for anime in resp.data.shows.edges {
    //     // Calculate episode count
    //     let ep_count = anime
    //         .available_episodes
    //         .get("sub")
    //         .and_then(|v| v.as_u64())
    //         .unwrap_or(0);
    //
    //     println!(
    //         "ID: {}\tName: {} ({} episodes)",
    //         anime.id, anime.name, ep_count
    //     );
    // }
    //
    if let Err(e) = get_episode_links(&client, "vDTSJHSpYnrkZnAvG", "1") {
        eprintln!("error: {}", e);
    }

    ratatui::run(app)?;
    Ok(())
}

fn app(terminal: &mut DefaultTerminal) -> std::io::Result<()> {
    loop {
        terminal.draw(render)?;
        if crossterm::event::read()?.is_key_press() {
            break Ok(());
        }
        // if let Event::Key(key) = event::read()? {
        //     match key.code {
        //         event::KeyCode::Esc => {
        //             break Ok(());
        //         }
        //         _ => (),
        //     }
        // }
    }
}

fn render(frame: &mut Frame) {
    let args = Args::parse();
    let config = Agent::config_builder()
        .timeout_per_call(Some(Duration::from_secs(12)))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Gecko/20100101 Firefox/121.0")
        .https_only(true)
        .build();

    let client = Agent::new_with_config(config);

    let mut row = vec![];
    let resp = search_anime(&client, &args.name).unwrap();
    for anime in resp.data.shows.edges {
        // Calculate episode count
        let ep_count = anime
            .available_episodes
            .get("sub")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);

        row.push(Row::new(vec![ep_count.to_string(), anime.name, anime.id]));
    }

    row.insert(
        0,
        Row::new(vec!["Episodes", "Name", "ID"]).style(Style::new().green()),
    );

    let outer_area = frame.area();
    let outer_block = Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().red())
        .title("shio")
        .title_alignment(HorizontalAlignment::Center);

    let inner_area = outer_block.inner(outer_area);
    let layout_inner = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(vec![Constraint::Percentage(70), Constraint::Percentage(30)])
        .split(inner_area);

    frame.render_widget(outer_block, outer_area);

    frame.render_widget(
        Table::new(
            row,
            [
                Constraint::Percentage(20),
                Constraint::Percentage(60),
                Constraint::Percentage(20),
            ],
        )
        .block(Block::bordered()),
        layout_inner[0],
    );

    frame.render_widget(
        Paragraph::new("test").block(Block::bordered()),
        layout_inner[1],
    );
}
