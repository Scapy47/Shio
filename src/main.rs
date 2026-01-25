use std::{collections::HashMap, time::Duration};

use clap::Parser;
use serde::{Deserialize, Serialize};
use serde_json::Value;
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
    let search_gql: &str = r#"
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

        // ACCESSING the fields here silences the warnings
        println!(
            "ID: {}\tName: {} ({} episodes)",
            anime.id,   // Reads 'id'
            anime.name, // Reads 'name'
            ep_count    // Reads 'available_episodes'
        );
    }

    Ok(())
}

fn main() {
    let args = Args::parse();

    let config = Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(12)))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Gecko/20100101 Firefox/121.0")
        .https_only(true)
        .build();

    let client = Agent::new_with_config(config);

    // println!("This is the start of {}!!", args.name);
    if let Err(e) = search_anime(&client, &args.name) {
        eprintln!("Error: {}", e);
    }
}
