use http::HeaderMap;
use serde::Deserialize;
use std::collections::HashMap;

/// A validation configuration.
#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    default: SiteConfig,
    sites: HashMap<String, SiteConfig>,
}

impl Config {
    /// Creates a configuration.
    pub fn new(default: SiteConfig, sites: HashMap<String, SiteConfig>) -> Self {
        Self { default, sites }
    }

    /// Returns a default configuration for websites.
    pub const fn default(&self) -> &SiteConfig {
        &self.default
    }

    /// Returns websites.
    pub const fn sites(&self) -> &HashMap<String, SiteConfig> {
        &self.sites
    }
}

/// A website configuration.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct SiteConfig {
    #[serde(with = "http_serde::header_map")]
    headers: HeaderMap,
    recursive: bool,
}

impl SiteConfig {
    /// Returns headers attached to HTTP requests.
    pub const fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// Returns whether we should validate the website recursively.
    pub const fn recursive(&self) -> bool {
        self.recursive
    }

    /// Sets whether we should validate the website recursively
    pub fn set_recursive(mut self, recursive: bool) -> Self {
        self.recursive = recursive;
        self
    }
}
