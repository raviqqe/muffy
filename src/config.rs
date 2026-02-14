mod error;
mod serde;

pub use self::{error::ConfigError, serde::compile_config};
use alloc::sync::Arc;
use core::{cmp::Reverse, ops::Deref, time::Duration};
use http::{HeaderMap, StatusCode};
use regex::Regex;
use rlimit::{Resource, getrlimit};
use std::collections::{HashMap, HashSet};
use url::Url;

type HostConfig = HashMap<String, Arc<SiteConfig>>;

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

const DEFAULT_MINIMUM_CONCURRENCY: usize = 256;

/// Returns a default concurrency.
pub fn default_concurrency() -> usize {
    getrlimit(Resource::NOFILE)
        .map(|(count, _)| (count / 2) as _)
        .unwrap_or(DEFAULT_MINIMUM_CONCURRENCY)
}

/// A validation configuration.
#[derive(Clone, Debug)]
pub struct Config {
    roots: Vec<String>,
    excluded_links: Vec<Regex>,
    default: Arc<SiteConfig>,
    sites: HashMap<String, Vec<(String, Arc<SiteConfig>)>>,
    concurrency: Option<usize>,
}

impl Config {
    /// Creates a configuration.
    pub fn new(
        roots: Vec<String>,
        default: Arc<SiteConfig>,
        sites: HashMap<String, HostConfig>,
        concurrency: Option<usize>,
    ) -> Self {
        Self {
            roots,
            excluded_links: Default::default(),
            default,
            sites: sites
                .into_iter()
                .map(|(host, value)| {
                    let mut paths = value.into_iter().collect::<Vec<_>>();
                    paths.sort_by_key(|(path, _)| Reverse(path.clone()));
                    (host, paths)
                })
                .collect(),
            concurrency,
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
    pub const fn sites(&self) -> &HashMap<String, Vec<(String, Arc<SiteConfig>)>> {
        &self.sites
    }

    /// Gets a site config
    pub fn site(&self, url: &Url) -> &SiteConfig {
        self.get_site(url).unwrap_or(&self.default)
    }

    /// Returns a concurrency.
    pub fn concurrency(&self) -> usize {
        self.concurrency.unwrap_or_else(default_concurrency)
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
            .find_map(|(path, config)| url.path().starts_with(path).then_some(config.as_ref()))
    }
}

/// A site configuration.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SiteConfig {
    headers: HeaderMap,
    max_age: Option<Duration>,
    max_redirects: usize,
    recursive: bool,
    retries: usize,
    scheme: SchemeConfig,
    status: StatusConfig,
    timeout: Option<Duration>,
}

impl SiteConfig {
    /// Creates a site configuration.
    pub fn new() -> Self {
        Self::default()
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
    pub const fn timeout(&self) -> Option<Duration> {
        self.timeout
    }

    /// Returns a maximum cache age.
    pub const fn max_age(&self) -> Option<Duration> {
        self.max_age
    }

    /// Returns a number of retries.
    pub const fn retries(&self) -> usize {
        self.retries
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
    pub const fn set_max_age(mut self, age: Option<Duration>) -> Self {
        self.max_age = age;
        self
    }

    /// Sets a timeout.
    pub const fn set_timeout(mut self, duration: Option<Duration>) -> Self {
        self.timeout = duration;
        self
    }

    /// Sets a number of retries.
    pub const fn set_retries(mut self, retries: usize) -> Self {
        self.retries = retries;
        self
    }

    /// Sets whether we should validate the website recursively
    pub const fn set_recursive(mut self, recursive: bool) -> Self {
        self.recursive = recursive;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn site_config_path_order() {
        let config = Config::new(
            vec![],
            Default::default(),
            [(
                "example.com".to_string(),
                [
                    (
                        "/foo".to_string(),
                        SiteConfig::new().set_recursive(true).into(),
                    ),
                    (
                        "/bar".to_string(),
                        SiteConfig::new().set_recursive(true).into(),
                    ),
                    (
                        "/".to_string(),
                        SiteConfig::new().set_recursive(false).into(),
                    ),
                    (
                        "/baz".to_string(),
                        SiteConfig::new().set_recursive(true).into(),
                    ),
                    (
                        "/qux".to_string(),
                        SiteConfig::new().set_recursive(true).into(),
                    ),
                ]
                .into_iter()
                .collect(),
            )]
            .into(),
            None,
        );

        assert!(
            config
                .site(&Url::parse("http://example.com/foo").unwrap())
                .recursive()
        );
        assert!(
            config
                .site(&Url::parse("http://example.com/bar").unwrap())
                .recursive()
        );
        assert!(
            config
                .site(&Url::parse("http://example.com/baz").unwrap())
                .recursive()
        );
        assert!(
            config
                .site(&Url::parse("http://example.com/qux").unwrap())
                .recursive()
        );
        assert!(
            !config
                .site(&Url::parse("http://example.com/other").unwrap())
                .recursive()
        );
    }
}
