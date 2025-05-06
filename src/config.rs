use core::ops::Deref;
use http::HeaderMap;
use serde::Deserialize;
use std::collections::HashMap;

type HostConfig = HashMap<u16, Vec<(String, SiteConfig)>>;

/// A validation configuration.
#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    roots: Vec<String>,
    default: SiteConfig,
    sites: HashMap<String, HostConfig>,
}

impl Config {
    /// Creates a configuration.
    pub const fn new(
        roots: Vec<String>,
        default: SiteConfig,
        sites: HashMap<String, HostConfig>,
    ) -> Self {
        Self {
            roots,
            default,
            sites,
        }
    }

    /// Returns root URLs.
    pub fn roots(&self) -> impl Iterator<Item = &str> {
        self.roots.iter().map(Deref::deref)
    }

    /// Returns a default configuration for websites.
    pub const fn default(&self) -> &SiteConfig {
        &self.default
    }

    /// Returns websites.
    pub const fn sites(&self) -> &HashMap<String, HostConfig> {
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
    pub const fn set_recursive(mut self, recursive: bool) -> Self {
        self.recursive = recursive;
        self
    }
}
