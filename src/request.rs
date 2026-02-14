use crate::{RetryConfig, http_client::BareRequest};
use alloc::sync::Arc;
use core::time::Duration;
use http::HeaderMap;
use url::Url;

#[derive(Clone, Debug)]
pub struct Request {
    bare: BareRequest,
    max_age: Option<Duration>,
    max_redirects: usize,
    retry: Arc<RetryConfig>,
    timeout: Option<Duration>,
}

impl Request {
    pub fn new(url: Url, headers: HeaderMap) -> Self {
        Self {
            bare: BareRequest { url, headers },
            max_age: Default::default(),
            max_redirects: Default::default(),
            retry: Default::default(),
            timeout: Default::default(),
        }
    }

    pub const fn as_bare(&self) -> &BareRequest {
        &self.bare
    }

    pub const fn url(&self) -> &Url {
        &self.bare.url
    }

    pub const fn max_redirects(&self) -> usize {
        self.max_redirects
    }

    pub fn timeout(&self) -> Duration {
        self.timeout.unwrap_or(Duration::MAX)
    }

    pub const fn max_age(&self) -> Option<Duration> {
        self.max_age
    }

    pub fn retry(&self) -> &RetryConfig {
        &self.retry
    }

    pub const fn set_max_age(mut self, max_age: Option<Duration>) -> Self {
        self.max_age = max_age;
        self
    }

    pub const fn set_max_redirects(mut self, max_redirects: usize) -> Self {
        self.max_redirects = max_redirects;
        self
    }

    pub const fn set_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn set_retry(mut self, config: Arc<RetryConfig>) -> Self {
        self.retry = config;
        self
    }

    pub fn set_url(mut self, url: Url) -> Self {
        self.bare.url = url;
        self
    }
}
