use http::HeaderMap;
use url::Url;

#[derive(Clone, Debug)]
pub struct Request {
    url: Url,
    headers: HeaderMap,
    max_redirects: usize,
}

impl Request {
    pub fn new(url: Url, headers: HeaderMap, max_redirects: usize) -> Self {
        Self {
            url,
            headers,
            max_redirects,
        }
    }

    pub fn url(&self) -> &Url {
        &self.url
    }

    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    pub fn max_redirects(&self) -> usize {
        self.max_redirects
    }

    pub fn with_url(&self, url: Url) -> Self {
        Self {
            url,
            ..self.clone()
        }
    }
}
