mod error;
mod serde;
mod toml;

pub use self::{
    error::ConfigError,
    serde::{SerializableConfig, compile_config},
    toml::read_config,
};
use alloc::sync::Arc;
use core::{cmp::Reverse, ops::Deref, time::Duration};
use http::{HeaderMap, StatusCode};
use regex::Regex;
use rlimit::{Resource, getrlimit};
use std::collections::{HashMap, HashSet};
use url::Url;

/// Default accepted URL schemes.
pub const DEFAULT_ACCEPTED_SCHEMES: &[&str] = &["http", "https"];
/// Default accepted HTTP status codes.
pub const DEFAULT_ACCEPTED_STATUS_CODES: &[StatusCode] = &[StatusCode::OK];
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
    ignored_links: Vec<Regex>,
    default: Arc<SiteConfig>,
    sites: HashMap<String, Vec<(String, Arc<SiteConfig>)>>,
    concurrency: ConcurrencyConfig,
    persistent_cache: bool,
    rate_limit: RateLimitConfig,
}

impl Config {
    /// Creates a configuration.
    pub fn new(
        roots: Vec<String>,
        default: Arc<SiteConfig>,
        sites: HashMap<String, HashMap<String, Arc<SiteConfig>>>,
    ) -> Self {
        Self {
            roots,
            ignored_links: Default::default(),
            default,
            sites: sites
                .into_iter()
                .map(|(host, value)| {
                    let mut paths = value.into_iter().collect::<Vec<_>>();
                    paths.sort_by_key(|(path, _)| Reverse(path.clone()));
                    (host, paths)
                })
                .collect(),
            concurrency: Default::default(),
            persistent_cache: false,
            rate_limit: Default::default(),
        }
    }

    /// Returns root URLs.
    pub fn roots(&self) -> impl Iterator<Item = &str> {
        self.roots.iter().map(Deref::deref)
    }

    /// Returns ignored link patterns.
    pub fn ignored_links(&self) -> impl Iterator<Item = &Regex> {
        self.ignored_links.iter()
    }

    /// Returns websites.
    pub const fn sites(&self) -> &HashMap<String, Vec<(String, Arc<SiteConfig>)>> {
        &self.sites
    }

    /// Gets a site config
    pub fn site(&self, url: &Url) -> &SiteConfig {
        self.get_site(url).unwrap_or(&self.default)
    }

    /// Returns concurrency.
    pub const fn concurrency(&self) -> &ConcurrencyConfig {
        &self.concurrency
    }

    /// Returns whether a cache is persistent.
    pub const fn persistent_cache(&self) -> bool {
        self.persistent_cache
    }

    /// Returns a rate limit.
    pub const fn rate_limit(&self) -> &RateLimitConfig {
        &self.rate_limit
    }

    /// Sets concurrency.
    pub fn set_concurrency(mut self, concurrency: ConcurrencyConfig) -> Self {
        self.concurrency = concurrency;
        self
    }

    /// Sets ignored link patterns.
    pub fn set_ignored_links(mut self, links: Vec<Regex>) -> Self {
        self.ignored_links = links;
        self
    }

    /// Sets persistent cache.
    pub const fn set_persistent_cache(mut self, persistent_cache: bool) -> Self {
        self.persistent_cache = persistent_cache;
        self
    }

    /// Sets a rate limit.
    pub fn set_rate_limit(mut self, rate_limit: RateLimitConfig) -> Self {
        self.rate_limit = rate_limit;
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
    id: Option<Arc<str>>,
    cache: CacheConfig,
    fragments_ignored: bool,
    headers: HeaderMap,
    max_redirects: usize,
    recursive: bool,
    retry: Arc<RetryConfig>,
    scheme: SchemeConfig,
    status: StatusConfig,
    timeout: Option<Duration>,
    validation: ValidationConfig,
}

impl SiteConfig {
    /// Creates a site configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns an ID.
    pub const fn id(&self) -> Option<&Arc<str>> {
        self.id.as_ref()
    }

    /// Returns a cache configuration.
    pub const fn cache(&self) -> &CacheConfig {
        &self.cache
    }

    /// Returns whether URL fragments should be ignored.
    pub const fn fragments_ignored(&self) -> bool {
        self.fragments_ignored
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

    /// Returns a validation configuration.
    pub const fn validation(&self) -> &ValidationConfig {
        &self.validation
    }

    /// Sets an ID.
    pub fn set_id(mut self, id: Option<Arc<str>>) -> Self {
        self.id = id;
        self
    }

    /// Sets a cache configuration.
    pub const fn set_cache(mut self, cache: CacheConfig) -> Self {
        self.cache = cache;
        self
    }

    /// Sets whether URL fragments are ignored.
    pub const fn set_fragments_ignored(mut self, ignored: bool) -> Self {
        self.fragments_ignored = ignored;
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

    /// Sets a validation configuration.
    pub fn set_validation(mut self, validation: ValidationConfig) -> Self {
        self.validation = validation;
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

/// A validation configuration.
#[derive(Clone, Debug, Eq, PartialEq, Default)]
pub struct ValidationConfig {
    html: Option<MarkupConfig>,
    svg: Option<MarkupConfig>,
    css: bool,
}

impl ValidationConfig {
    /// Returns an HTML validation configuration.
    pub const fn html(&self) -> Option<&MarkupConfig> {
        self.html.as_ref()
    }

    /// Returns an SVG validation configuration.
    pub const fn svg(&self) -> Option<&MarkupConfig> {
        self.svg.as_ref()
    }

    /// Returns whether CSS validation is enabled.
    pub const fn css(&self) -> bool {
        self.css
    }

    /// Sets an HTML validation configuration.
    pub fn set_html(mut self, config: Option<MarkupConfig>) -> Self {
        self.html = config;
        self
    }

    /// Sets an SVG validation configuration.
    pub fn set_svg(mut self, config: Option<MarkupConfig>) -> Self {
        self.svg = config;
        self
    }

    /// Sets whether CSS validation is enabled.
    pub const fn set_css(mut self, enabled: bool) -> Self {
        self.css = enabled;
        self
    }
}

/// A markup validation configuration.
#[derive(Clone, Debug, Default)]
pub struct MarkupConfig {
    ignored_attributes: Vec<Regex>,
    ignored_elements: Vec<Regex>,
}

impl MarkupConfig {
    /// Creates a markup validation configuration.
    pub const fn new(ignored_attributes: Vec<Regex>, ignored_elements: Vec<Regex>) -> Self {
        Self {
            ignored_attributes,
            ignored_elements,
        }
    }

    /// Returns ignored attributes.
    pub fn ignored_attributes(&self) -> &[Regex] {
        &self.ignored_attributes
    }

    /// Returns ignored elements.
    pub fn ignored_elements(&self) -> &[Regex] {
        &self.ignored_elements
    }
}

impl PartialEq for MarkupConfig {
    fn eq(&self, other: &Self) -> bool {
        self.ignored_attributes.len() == other.ignored_attributes.len()
            && self.ignored_elements.len() == other.ignored_elements.len()
            && self
                .ignored_attributes
                .iter()
                .zip(&other.ignored_attributes)
                .chain(self.ignored_elements.iter().zip(&other.ignored_elements))
                .all(|(one, other)| one.as_str() == other.as_str())
    }
}

impl Eq for MarkupConfig {}

/// A cache configuration.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CacheConfig {
    max_age: Duration,
}

impl CacheConfig {
    /// Creates a cache configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a maximum age.
    pub const fn max_age(&self) -> Duration {
        self.max_age
    }

    /// Sets a maximum age.
    pub const fn set_max_age(mut self, age: Duration) -> Self {
        self.max_age = age;
        self
    }
}

/// A retry configuration.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RetryConfig {
    count: usize,
    factor: f64,
    interval: RetryDurationConfig,
    status_codes: Vec<u16>,
}

impl RetryConfig {
    /// Creates a configuration.
    pub fn new() -> Self {
        Self {
            count: 0,
            factor: 1.0,
            interval: Default::default(),
            status_codes: Default::default(),
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
    pub const fn interval(&self) -> &RetryDurationConfig {
        &self.interval
    }

    /// Returns a list of status codes.
    pub fn status_codes(&self) -> &[u16] {
        &self.status_codes
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
    pub const fn set_interval(mut self, duration: RetryDurationConfig) -> Self {
        self.interval = duration;
        self
    }

    /// Sets a list of status codes.
    pub fn set_status_codes(mut self, status_codes: Vec<u16>) -> Self {
        self.status_codes = status_codes;
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

/// A concurrency configuration.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ConcurrencyConfig {
    global: Option<usize>,
    sites: HashMap<String, usize>,
}

impl ConcurrencyConfig {
    /// Creates a configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a global concurrency.
    pub const fn global(&self) -> Option<usize> {
        self.global
    }

    /// Returns concurrency per site.
    pub const fn sites(&self) -> &HashMap<String, usize> {
        &self.sites
    }

    /// Sets a global concurrency.
    pub const fn set_global(mut self, concurrency: Option<usize>) -> Self {
        self.global = concurrency;
        self
    }

    /// Sets concurrency per site.
    pub fn set_sites(mut self, sites: HashMap<String, usize>) -> Self {
        self.sites = sites;
        self
    }
}

/// A rate limit configuration.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RateLimitConfig {
    global: Option<SiteRateLimitConfig>,
    sites: HashMap<String, SiteRateLimitConfig>,
}

impl RateLimitConfig {
    /// Creates a configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a global rate limit.
    pub const fn global(&self) -> Option<&SiteRateLimitConfig> {
        self.global.as_ref()
    }

    /// Returns rate limits per site.
    pub const fn sites(&self) -> &HashMap<String, SiteRateLimitConfig> {
        &self.sites
    }

    /// Sets a global rate limit.
    pub const fn set_global(mut self, rate_limit: Option<SiteRateLimitConfig>) -> Self {
        self.global = rate_limit;
        self
    }

    /// Sets rate limits per site.
    pub fn set_sites(mut self, sites: HashMap<String, SiteRateLimitConfig>) -> Self {
        self.sites = sites;
        self
    }
}

/// A site rate limit configuration.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SiteRateLimitConfig {
    supply: u64,
    window: Duration,
}

impl SiteRateLimitConfig {
    /// Creates a configuration.
    pub const fn new(supply: u64, window: Duration) -> Self {
        Self { supply, window }
    }

    /// Returns a supply.
    pub const fn supply(&self) -> u64 {
        self.supply
    }

    /// Returns a window.
    pub const fn window(&self) -> Duration {
        self.window
    }

    /// Sets a supply.
    pub const fn set_supply(mut self, supply: u64) -> Self {
        self.supply = supply;
        self
    }

    /// Sets a window.
    pub const fn set_window(mut self, window: Duration) -> Self {
        self.window = window;
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
                        SiteConfig::default()
                            .set_id(Some("foo".into()))
                            .set_recursive(true)
                            .into(),
                    ),
                    (
                        "/bar".to_string(),
                        SiteConfig::default()
                            .set_id(Some("bar".into()))
                            .set_recursive(true)
                            .into(),
                    ),
                    (
                        "/".to_string(),
                        SiteConfig::default()
                            .set_id(Some("top".into()))
                            .set_recursive(false)
                            .into(),
                    ),
                    (
                        "/baz".to_string(),
                        SiteConfig::default()
                            .set_id(Some("baz".into()))
                            .set_recursive(true)
                            .into(),
                    ),
                    (
                        "/qux".to_string(),
                        SiteConfig::default()
                            .set_id(Some("qux".into()))
                            .set_recursive(true)
                            .into(),
                    ),
                ]
                .into_iter()
                .collect(),
            )]
            .into(),
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

    #[test]
    fn default_validation_config() {
        let config = ValidationConfig::default();

        assert!(config.html().is_none());
        assert!(config.svg().is_none());
        assert!(!config.css());
    }

    #[test]
    fn set_validation_config_enabled() {
        let config = ValidationConfig::default()
            .set_html(Some(MarkupConfig::default()))
            .set_svg(Some(MarkupConfig::default()))
            .set_css(true);

        assert!(config.html().is_some());
        assert!(config.svg().is_some());
        assert!(config.css());
    }

    #[test]
    fn validate_site_config() {
        let config = SiteConfig::default();

        assert!(config.validation().html().is_none());
        assert!(
            config
                .set_validation(ValidationConfig::default().set_html(Some(MarkupConfig::default())))
                .validation()
                .html()
                .is_some()
        );
    }
}
