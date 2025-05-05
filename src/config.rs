use http::HeaderMap;
use serde::Deserialize;
use std::collections::HashMap;

/// A validation configuration.
#[derive(Debug, Deserialize)]
pub struct Config {
    default: Site,
    sites: HashMap<String, Site>,
}

impl Config {
    /// Returns a default configuration for websites.
    pub fn default(&self) -> &Site {
        &self.default
    }

    /// Returns websites.
    pub fn sites(&self) -> &HashMap<String, Site> {
        &self.sites
    }
}

/// A website configuration.
#[derive(Debug, Deserialize)]
pub struct Site {
    #[serde(with = "http_serde::header_map")]
    headers: HeaderMap,
    recurse: bool,
}

impl Site {
    /// Returns headers attached to HTTP requests.
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }
}
