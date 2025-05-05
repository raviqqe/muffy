use http::HeaderMap;
use serde::Deserialize;
use std::collections::HashMap;

/// A validation configuration.
#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    default: Site,
    sites: HashMap<String, Site>,
}

impl Config {
    /// Returns a default configuration for websites.
    pub const fn default(&self) -> &Site {
        &self.default
    }

    /// Returns websites.
    pub const fn sites(&self) -> &HashMap<String, Site> {
        &self.sites
    }
}

/// A website configuration.
#[derive(Debug, Deserialize)]
pub struct Site {
    #[serde(with = "http_serde::header_map")]
    headers: HeaderMap,
    recursive: bool,
}

impl Site {
    /// Returns headers attached to HTTP requests.
    pub const fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// Returns whether we should validate the website recursively.
    pub const fn recursive(&self) -> bool {
        self.recursive
    }
}
