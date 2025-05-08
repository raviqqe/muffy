use crate::http_client::BareRequest;
use http::HeaderMap;
use std::time::Duration;
use url::Url;

#[derive(Clone, Debug)]
pub struct Request {
    bare: BareRequest,
    max_redirects: usize,
    max_age: Duration,
}

impl Request {
    pub const fn new(
        url: Url,
        headers: HeaderMap,
        max_redirects: usize,
        max_age: Duration,
    ) -> Self {
        Self {
            bare: BareRequest { url, headers },
            max_redirects,
            max_age,
        }
    }

    pub const fn url(&self) -> &Url {
        &self.bare.url
    }

    pub const fn max_redirects(&self) -> usize {
        self.max_redirects
    }

    pub const fn max_age(&self) -> Duration {
        self.max_age
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
