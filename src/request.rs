use crate::http_client::BareRequest;
use core::time::Duration;
use http::HeaderMap;
use std::time::SystemTime;
use url::Url;

#[derive(Clone, Debug)]
pub struct Request {
    bare: BareRequest,
    max_redirects: usize,
    timeout: Option<Duration>,
    expiry: Option<SystemTime>,
}

impl Request {
    pub fn new(url: Url, headers: HeaderMap) -> Self {
        Self {
            bare: BareRequest { url, headers },
            max_redirects: Default::default(),
            timeout: Default::default(),
            expiry: Default::default(),
        }
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

    pub const fn expiry(&self) -> Option<SystemTime> {
        self.expiry
    }

    pub const fn set_max_redirects(mut self, max_redirects: usize) -> Self {
        self.max_redirects = max_redirects;
        self
    }

    pub const fn set_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        self
    }

    pub const fn set_expiry(mut self, expiry: Option<SystemTime>) -> Self {
        self.expiry = expiry;
        self
    }

    pub const fn as_bare(&self) -> &BareRequest {
        &self.bare
    }

    // TODO Rename this to `set_url`?
    pub fn with_url(&self, url: Url) -> Self {
        Self {
            bare: BareRequest {
                url,
                ..self.bare.clone()
            },
            ..self.clone()
        }
    }
}
