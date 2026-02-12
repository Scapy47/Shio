use clap::ValueEnum;
use serde::Deserialize;
use serde_json::Value;
use std::{collections::HashMap, time::Duration};
use ureq::{Agent, RequestBuilder, typestate::WithoutBody};

use crate::decrypt_url;

//  NOTE: Response from search_anime()
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct AnimeEdge {
    #[serde(rename = "_id")]
    pub id: String,
    pub name: String,

    pub english_name: Option<String>,
    pub available_episodes: Option<HashMap<String, Value>>,
    #[serde(rename = "__typename")]
    pub typename: String,
}

#[derive(Deserialize, Debug)]
pub struct ShowsData {
    pub edges: Vec<AnimeEdge>,
}

#[derive(Deserialize, Debug)]
pub struct DataWrapper {
    pub shows: ShowsData,
}

#[derive(Deserialize, Debug)]
pub struct SearchResponse {
    pub data: DataWrapper,
}

//  NOTE: Response for get_episode_links()
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SourceUrl {
    pub source_url: String,
    pub source_name: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeData {
    pub episode_string: String,
    pub source_urls: Vec<SourceUrl>,
}

#[derive(Deserialize, Debug)]
pub struct EpisodeDataWrapper {
    pub episode: EpisodeData,
}

#[derive(Deserialize, Debug)]
pub struct EpisodeResponse {
    pub data: EpisodeDataWrapper,
}

//  NOTE: Response for get_episode_list()
#[derive(Deserialize, Debug)]
pub struct ShowDetail {
    #[serde(rename = "_id")]
    pub id: String,
    pub name: String,
    #[serde(rename = "availableEpisodesDetail")]
    pub available_episodes_detail: HashMap<String, Vec<String>>,
}

#[derive(Deserialize, Debug)]
pub struct ShowDetailData {
    pub show: ShowDetail,
}

#[derive(Deserialize, Debug)]
pub struct EpisodeListResponse {
    pub data: ShowDetailData,
}

#[derive(Debug)]
pub struct Api {
    pub base_api: String,
    pub referer: String,
    pub agent: Agent,
    pub user_agent: String,
    pub mode: String,
    pub debug: bool,
}

#[derive(ValueEnum, Debug, Clone, Copy)]
pub enum Mode {
    Sub,
    Dub,
    Raw,
}

impl Api {
    pub fn new(mode: Mode, debug: bool) -> Self {
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
    pub fn search_anime(
        &self,
        query: String,
    ) -> Result<SearchResponse, Box<dyn std::error::Error>> {
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
    pub fn get_episode_links(
        &self,
        id: &str,
        ep: &str,
    ) -> Result<(String, Vec<(String, String)>), Box<dyn std::error::Error>> {
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
                format!("https:{}", raw_uri)
            } else {
                raw_uri
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

            let uri = if uri.contains("clock.json") {
                self.resolve_clock_urls(&uri).unwrap_or(uri)
            } else {
                uri
            };

            if self.debug {
                unimplemented!()
            }

            vec.push((provider_name, uri));
        }

        Ok((parsed.data.episode.episode_string, vec))
    }

    fn resolve_clock_urls(&self, url: &str) -> Result<String, Box<dyn std::error::Error>> {
        let resp = self.agent.get(url).call()?;
        let json: serde_json::Value = resp.into_body().read_json()?;

        if let Some(links_array) = json["links"].as_array() {
            if let Some(first_item) = links_array.first() {
                if let Some(wixmp_url) = first_item["link"].as_str() {
                    return Ok(wixmp_url.to_string());
                }
            }
        }

        Err("Could not find 'link' field in clock.json response".into())
    }

    /// Get list of episodes available from api
    pub fn get_episode_list(
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
            unimplemented!()
        }

        Ok((parsed.data.show.name, episodes, parsed.data.show.id))
    }
}
