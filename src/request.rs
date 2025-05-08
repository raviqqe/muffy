use http::HeaderMap;
use url::Url;

#[derive(Clone, Debug)]
pub struct Request {
    url: Url,
    headers: HeaderMap,
    max_redirects: usize,
}

impl Request {
    pub const fn new(url: Url, headers: HeaderMap, max_redirects: usize) -> Self {
        Self {
            url,
            headers,
            max_redirects,
        }
    }

    pub const fn url(&self) -> &Url {
        &self.url
    }

    pub const fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    pub const fn max_redirects(&self) -> usize {
        self.max_redirects
    }

    pub fn with_url(&self, url: Url) -> Self {
        Self {
            url,
            ..self.clone()
        }
    }
}
