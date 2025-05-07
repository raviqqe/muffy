use crate::default_port;
use core::ops::Deref;
use http::{HeaderMap, StatusCode};
use std::collections::{HashMap, HashSet};
use url::Url;

type HostConfig = HashMap<u16, Vec<(String, SiteConfig)>>;

/// A validation configuration.
#[derive(Clone, Debug)]
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

    /// Returns websites.
    pub const fn sites(&self) -> &HashMap<String, HostConfig> {
        &self.sites
    }

    /// Gets a site config
    pub fn site(&self, url: &Url) -> &SiteConfig {
        self.get_site(url).unwrap_or(&self.default)
    }

    fn get_site(&self, url: &Url) -> Option<&SiteConfig> {
        self.sites()
            .get(url.host_str()?)?
            .get(&url.port().unwrap_or_else(|| default_port(url)))?
            .iter()
            .find_map(|(path, config)| url.path().starts_with(path).then_some(config))
    }
}

/// A site configuration.
#[derive(Clone, Debug, Default)]
pub struct SiteConfig {
    headers: HeaderMap,
    status: StatusConfig,
    recursive: bool,
}

impl SiteConfig {
    /// Creates a site configuration.
    pub const fn new(headers: HeaderMap, status: StatusConfig, recursive: bool) -> Self {
        Self {
            headers,
            status,
            recursive,
        }
    }

    /// Returns headers attached to HTTP requests.
    pub const fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// Returns a status configuration.
    pub const fn status(&self) -> &StatusConfig {
        &self.status
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

#[derive(Clone, Debug)]
pub struct StatusConfig {
    accepted: HashSet<StatusCode>,
}

impl StatusConfig {
    pub const fn new(accepted: HashSet<StatusCode>) -> Self {
        Self { accepted }
    }

    pub fn accepted(&self, status: StatusCode) -> bool {
        self.accepted.contains(&status)
    }
}

impl Default for StatusConfig {
    fn default() -> Self {
        Self {
            accepted: HashSet::from_iter([StatusCode::OK]),
        }
    }
}
