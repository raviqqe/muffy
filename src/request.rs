use crate::http_client::BareRequest;
use core::time::Duration;
use http::HeaderMap;
use url::Url;

#[derive(Clone, Debug)]
pub struct Request {
    bare: BareRequest,
    max_redirects: usize,
    timeout: Option<Duration>,
    max_age: Duration,
}

impl Request {
    pub fn new(url: Url, headers: HeaderMap) -> Self {
        Self {
            bare: BareRequest { url, headers },
            max_redirects: Default::default(),
            timeout: Default::default(),
            max_age: Default::default(),
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

    pub const fn max_age(&self) -> Duration {
        self.max_age
    }

    pub fn set_max_redirects(mut self, max_redirects: usize) -> Self {
        self.max_redirects = max_redirects;
        self
    }

    pub fn set_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn set_max_age(mut self, max_age: Duration) -> Self {
        self.max_age = max_age;
        self
    }

    pub const fn as_bare(&self) -> &BareRequest {
        &self.bare
    }

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
