mod error;
mod serde;

pub use self::{error::ConfigError, serde::compile_config};
use core::{ops::Deref, time::Duration};
use http::{HeaderMap, StatusCode};
use regex::Regex;
use std::collections::{HashMap, HashSet};
use url::Url;

type HostConfig = Vec<(String, SiteConfig)>;

/// Default accepted URL schemes.
pub const DEFAULT_ACCEPTED_SCHEMES: &[&str] = &["http", "https"];
/// Default accepted HTTP status codes.
pub const DEFAULT_ACCEPTED_STATUS_CODES: &[StatusCode] = &[StatusCode::OK];
/// A default maximum cache age.
pub const DEFAULT_MAX_CACHE_AGE: Duration = Duration::from_secs(3600);
/// A default number of maximum redirects.
pub const DEFAULT_MAX_REDIRECTS: usize = 16;
/// A default HTTP timeout.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// A validation configuration.
#[derive(Clone, Debug)]
pub struct Config {
    roots: Vec<String>,
    excluded_links: Vec<Regex>,
    default: SiteConfig,
    sites: HashMap<String, HostConfig>,
}

impl Config {
    /// Creates a configuration.
    pub fn new(
        roots: Vec<String>,
        default: SiteConfig,
        sites: HashMap<String, HostConfig>,
    ) -> Self {
        Self {
            roots,
            excluded_links: Default::default(),
            default,
            sites,
        }
    }

    /// Returns root URLs.
    pub fn roots(&self) -> impl Iterator<Item = &str> {
        self.roots.iter().map(Deref::deref)
    }

    /// Returns excluded link patterns.
    pub fn excluded_links(&self) -> impl Iterator<Item = &Regex> {
        self.excluded_links.iter()
    }

    /// Returns websites.
    pub const fn sites(&self) -> &HashMap<String, HostConfig> {
        &self.sites
    }

    /// Gets a site config
    pub fn site(&self, url: &Url) -> &SiteConfig {
        self.get_site(url).unwrap_or(&self.default)
    }

    /// Set excluded link patterns.
    pub fn set_excluded_links(mut self, links: Vec<Regex>) -> Self {
        self.excluded_links = links;
        self
    }

    fn get_site(&self, url: &Url) -> Option<&SiteConfig> {
        self.sites()
            .get(url.host_str()?)?
            .iter()
            .find_map(|(path, config)| url.path().starts_with(path).then_some(config))
    }
}

/// A site configuration.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SiteConfig {
    headers: HeaderMap,
    status: StatusConfig,
    scheme: SchemeConfig,
    max_redirects: usize,
    timeout: Option<Duration>,
    max_age: Duration,
    recursive: bool,
}

impl SiteConfig {
    /// Creates a site configuration.
    pub const fn new(
        headers: HeaderMap,
        status: StatusConfig,
        scheme: SchemeConfig,
        max_redirects: usize,
        timeout: Option<Duration>,
        max_age: Duration,
        recursive: bool,
    ) -> Self {
        Self {
            headers,
            status,
            scheme,
            max_redirects,
            timeout,
            max_age,
            recursive,
        }
    }

    /// Returns headers attached to HTTP requests.
    pub const fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// Returns a status code configuration.
    pub const fn status(&self) -> &StatusConfig {
        &self.status
    }

    /// Returns a scheme configuration.
    pub const fn scheme(&self) -> &SchemeConfig {
        &self.scheme
    }

    /// Returns a maximum number of redirects.
    pub const fn max_redirects(&self) -> usize {
        self.max_redirects
    }

    /// Returns a timeout.
    pub fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    /// Returns a maximum cache age.
    pub const fn max_age(&self) -> Duration {
        self.max_age
    }

    /// Returns whether we should validate the website recursively.
    pub const fn recursive(&self) -> bool {
        self.recursive
    }

    /// Sets request headers.
    pub fn set_headers(mut self, headers: HeaderMap) -> Self {
        self.headers = headers;
        self
    }

    /// Sets a status code configuration.
    pub fn set_status(mut self, status: StatusConfig) -> Self {
        self.status = status;
        self
    }

    /// Sets a scheme configuration.
    pub fn set_scheme(mut self, scheme: SchemeConfig) -> Self {
        self.scheme = scheme;
        self
    }

    /// Sets a maximum number of redirects.
    pub const fn set_max_redirects(mut self, count: usize) -> Self {
        self.max_redirects = count;
        self
    }

    /// Sets a maximum cache age.
    pub const fn set_max_age(mut self, age: Duration) -> Self {
        self.max_age = age;
        self
    }

    /// Sets whether we should validate the website recursively
    pub const fn set_recursive(mut self, recursive: bool) -> Self {
        self.recursive = recursive;
        self
    }

    /// Sets an HTTP timeout.
    pub const fn set_timeout(mut self, duration: Option<Duration>) -> Self {
        self.timeout = duration;
        self
    }
}

/// A status code configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StatusConfig {
    accepted: HashSet<StatusCode>,
}

impl StatusConfig {
    /// Creates a status code configuration.
    pub const fn new(accepted: HashSet<StatusCode>) -> Self {
        Self { accepted }
    }

    /// Returns whether a status code is accepted.
    pub fn accepted(&self, status: StatusCode) -> bool {
        self.accepted.contains(&status)
    }
}

impl Default for StatusConfig {
    fn default() -> Self {
        Self {
            accepted: DEFAULT_ACCEPTED_STATUS_CODES.iter().copied().collect(),
        }
    }
}

/// A scheme configuration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchemeConfig {
    accepted: HashSet<String>,
}

impl SchemeConfig {
    /// Creates a scheme configuration.
    pub const fn new(accepted: HashSet<String>) -> Self {
        Self { accepted }
    }

    /// Returns whether a scheme is accepted.
    pub fn accepted(&self, scheme: &str) -> bool {
        self.accepted.contains(scheme)
    }
}

impl Default for SchemeConfig {
    fn default() -> Self {
        Self {
            accepted: DEFAULT_ACCEPTED_SCHEMES
                .iter()
                .copied()
                .map(ToOwned::to_owned)
                .collect(),
        }
    }
}
