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
    persistent_cache: bool,
}

impl Config {
    /// Creates a configuration.
    pub fn new(
        roots: Vec<String>,
        default: Arc<SiteConfig>,
        sites: HashMap<String, HostConfig>,
        concurrency: Option<usize>,
        persistent_cache: bool,
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
            persistent_cache,
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

    /// Returns whether a cache is persistent.
    pub const fn persistent_cache(&self) -> bool {
        self.persistent_cache
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
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SiteConfig {
    cache: CacheConfig,
    concurrency: Option<usize>,
    headers: HeaderMap,
    max_redirects: usize,
    recursive: bool,
    retry: Arc<RetryConfig>,
    scheme: SchemeConfig,
    status: StatusConfig,
    timeout: Option<Duration>,
}

impl SiteConfig {
    /// Creates a site configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a cache configuration.
    pub const fn cache(&self) -> &CacheConfig {
        &self.cache
    }

    /// Returns a concurrency.
    pub fn concurrency(&self) -> Option<usize> {
        self.concurrency
    }

    /// Returns headers attached to HTTP requests.
    pub const fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// Returns a retry configuration.
    pub const fn retry(&self) -> &Arc<RetryConfig> {
        &self.retry
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

    /// Returns whether we should validate the website recursively.
    pub const fn recursive(&self) -> bool {
        self.recursive
    }

    /// Sets a cache configuration.
    pub const fn set_cache(mut self, cache: CacheConfig) -> Self {
        self.cache = cache;
        self
    }

    /// Sets a concurrency.
    pub const fn set_concurrency(mut self, concurrency: Option<usize>) -> Self {
        self.concurrency = concurrency;
        self
    }

    /// Sets request headers.
    pub fn set_headers(mut self, headers: HeaderMap) -> Self {
        self.headers = headers;
        self
    }

    /// Sets a retry configuration.
    pub fn set_retry(mut self, retry: Arc<RetryConfig>) -> Self {
        self.retry = retry;
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

    /// Sets a timeout.
    pub const fn set_timeout(mut self, duration: Option<Duration>) -> Self {
        self.timeout = duration;
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

/// A cache configuration.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CacheConfig {
    max_age: Option<Duration>,
}

impl CacheConfig {
    /// Creates a cache configuration.
    pub const fn new() -> Self {
        Self { max_age: None }
    }

    /// Returns a maximum age.
    pub const fn max_age(&self) -> Option<Duration> {
        self.max_age
    }

    /// Sets a maximum age.
    pub const fn set_max_age(mut self, age: Option<Duration>) -> Self {
        self.max_age = age;
        self
    }
}

/// A retry configuration.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RetryConfig {
    count: usize,
    factor: f64,
    duration: RetryDurationConfig,
}

impl RetryConfig {
    /// Creates a configuration.
    pub fn new() -> Self {
        Self {
            count: 0,
            factor: 1.0,
            duration: Default::default(),
        }
    }

    /// Returns a count.
    pub const fn count(&self) -> usize {
        self.count
    }

    /// Returns a factor.
    pub const fn factor(&self) -> f64 {
        self.factor
    }

    /// Returns a duration configuration.
    pub const fn duration(&self) -> &RetryDurationConfig {
        &self.duration
    }

    /// Sets a count.
    pub const fn set_count(mut self, count: usize) -> Self {
        self.count = count;
        self
    }

    /// Sets a factor.
    pub const fn set_factor(mut self, factor: f64) -> Self {
        self.factor = factor;
        self
    }

    /// Sets a duration configuration.
    pub const fn set_duration(mut self, duration: RetryDurationConfig) -> Self {
        self.duration = duration;
        self
    }
}

/// A retry duration configuration.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RetryDurationConfig {
    initial: Duration,
    cap: Option<Duration>,
}

impl RetryDurationConfig {
    /// Creates a configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns an initial duration.
    pub const fn initial(&self) -> Duration {
        self.initial
    }

    /// Returns a cap duration.
    pub const fn cap(&self) -> Option<Duration> {
        self.cap
    }

    /// Sets an initial duration.
    pub const fn set_initial(mut self, duration: Duration) -> Self {
        self.initial = duration;
        self
    }

    /// Sets a cap duration.
    pub const fn set_cap(mut self, duration: Option<Duration>) -> Self {
        self.cap = duration;
        self
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
            false,
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
