use crate::{RetryConfig, http_client::BareRequest};
use alloc::sync::Arc;
use core::time::Duration;
use http::{
    HeaderMap,
    header::{AUTHORIZATION, COOKIE, PROXY_AUTHORIZATION},
};
use url::Url;

#[derive(Clone, Debug)]
pub struct Request {
    bare: BareRequest,
    max_age: Duration,
    max_redirects: usize,
    retry: Arc<RetryConfig>,
    site_id: Option<Arc<str>>,
    stale_while_revalidate: Duration,
    timeout: Option<Duration>,
}

impl Request {
    pub fn new(url: Url, headers: HeaderMap) -> Self {
        Self {
            bare: BareRequest { url, headers },
            site_id: None,
            max_age: Default::default(),
            max_redirects: Default::default(),
            retry: Default::default(),
            stale_while_revalidate: Default::default(),
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

    pub fn site_id(&self) -> Option<&str> {
        self.site_id.as_deref()
    }

    pub fn timeout(&self) -> Duration {
        self.timeout.unwrap_or(Duration::MAX)
    }

    pub const fn max_age(&self) -> Duration {
        self.max_age
    }

    pub const fn stale_while_revalidate(&self) -> Duration {
        self.stale_while_revalidate
    }

    pub fn retry(&self) -> &RetryConfig {
        &self.retry
    }

    pub const fn set_max_age(mut self, max_age: Duration) -> Self {
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

    pub fn set_site_id(mut self, site_id: Option<Arc<str>>) -> Self {
        self.site_id = site_id;
        self
    }

    pub const fn set_stale_while_revalidate(mut self, duration: Duration) -> Self {
        self.stale_while_revalidate = duration;
        self
    }

    pub fn set_url(mut self, url: Url) -> Self {
        self.bare.url = url;
        self
    }

    pub fn redirect(mut self, url: Url) -> Self {
        if url.origin() != self.bare.url.origin() {
            for name in [AUTHORIZATION, COOKIE, PROXY_AUTHORIZATION] {
                self.bare.headers.remove(name);
            }
        }

        self.set_url(url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::{HeaderValue, header::ACCEPT};
    use pretty_assertions::assert_eq;

    fn credentialed_request(url: &str) -> Request {
        Request::new(
            Url::parse(url).unwrap(),
            [
                (AUTHORIZATION, HeaderValue::from_static("secret")),
                (COOKIE, HeaderValue::from_static("session=abc")),
                (ACCEPT, HeaderValue::from_static("text/html")),
            ]
            .into_iter()
            .collect(),
        )
    }

    #[test]
    fn strip_credentials_on_cross_origin_redirect() {
        let request = credentialed_request("https://foo.com/page")
            .redirect(Url::parse("https://bar.com/page").unwrap());

        assert_eq!(request.url().as_str(), "https://bar.com/page");
        assert_eq!(request.as_bare().headers.get(AUTHORIZATION), None);
        assert_eq!(request.as_bare().headers.get(COOKIE), None);
    }

    #[test]
    fn keep_credentials_on_same_origin_redirect() {
        let request = credentialed_request("https://foo.com/page")
            .redirect(Url::parse("https://foo.com/other").unwrap());

        assert_eq!(
            request.as_bare().headers.get(AUTHORIZATION),
            Some(&HeaderValue::from_static("secret"))
        );
        assert_eq!(
            request.as_bare().headers.get(COOKIE),
            Some(&HeaderValue::from_static("session=abc"))
        );
    }

    #[test]
    fn strip_credentials_on_scheme_downgrade() {
        let request = credentialed_request("https://foo.com/page")
            .redirect(Url::parse("http://foo.com/page").unwrap());

        assert_eq!(request.as_bare().headers.get(AUTHORIZATION), None);
    }

    #[test]
    fn strip_credentials_on_port_change() {
        let request = credentialed_request("https://foo.com/page")
            .redirect(Url::parse("https://foo.com:8443/page").unwrap());

        assert_eq!(request.as_bare().headers.get(AUTHORIZATION), None);
    }

    #[test]
    fn keep_other_header_on_cross_origin_redirect() {
        let request = credentialed_request("https://foo.com/page")
            .redirect(Url::parse("https://bar.com/page").unwrap());

        assert_eq!(
            request.as_bare().headers.get(ACCEPT),
            Some(&HeaderValue::from_static("text/html"))
        );
    }
}
