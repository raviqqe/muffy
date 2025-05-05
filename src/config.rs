use http::HeaderMap;
use serde::Deserialize;
use std::collections::HashMap;

/// A validation configuration.
#[derive(Debug, Deserialize)]
pub struct Config {
    sites: HashMap<String, Site>,
}

impl Config {
    pub fn sites(&self) -> &HashMap<String, Site> {
        &self.sites
    }
}

/// A website configuration.
pub struct Site {
    #[serde(with = "http_serde::header_map")]
    headers: HeaderMap,
}
