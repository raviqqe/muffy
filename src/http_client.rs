mod bare;
mod cached_response;
mod error;
mod reqwest;
#[cfg(test)]
mod stub;

#[cfg(test)]
pub use self::stub::{StubHttpClient, StubSequenceHttpClient, build_stub_response};
pub use self::{
    bare::{BareHttpClient, BareRequest, BareResponse},
    error::HttpClientError,
    reqwest::ReqwestHttpClient,
};
use crate::{cache::Cache, request::Request, response::Response, timer::Timer};
use alloc::sync::Arc;
use async_recursion::async_recursion;
use cached_response::CachedResponse;
use core::str;
use robotxt::Robots;
use tokio::{
    sync::Semaphore,
    time::{sleep, timeout},
};

const USER_AGENT: &str = "muffy";

/// A full-featured HTTP client.
pub struct HttpClient(Arc<HttpClientInner>);

struct HttpClientInner {
    client: Box<dyn BareHttpClient>,
    timer: Box<dyn Timer>,
    cache: Box<dyn Cache<Result<Arc<CachedResponse>, HttpClientError>>>,
    semaphore: Semaphore,
}

impl HttpClient {
    /// Creates an HTTP client.
    pub fn new(
        client: impl BareHttpClient + 'static,
        timer: impl Timer + 'static,
        cache: Box<dyn Cache<Result<Arc<CachedResponse>, HttpClientError>>>,
        concurrency: usize,
    ) -> Self {
        Self(
            HttpClientInner {
                client: Box::new(client),
                timer: Box::new(timer),
                cache,
                semaphore: Semaphore::new(concurrency),
            }
            .into(),
        )
    }

    fn cloned(&self) -> Self {
        Self(self.0.clone())
    }

    pub(crate) async fn get(
        &self,
        request: &Request,
    ) -> Result<Option<Arc<Response>>, HttpClientError> {
        match self.get_inner(request, true).await {
            Ok(response) => Ok(Some(response)),
            Err(HttpClientError::RobotsTxt) => Ok(None),
            Err(error) => Err(error),
        }
    }

    async fn get_inner(
        &self,
        request: &Request,
        robots: bool,
    ) -> Result<Arc<Response>, HttpClientError> {
        let mut request = request.clone();

        for _ in 0..request.max_redirects() + 1 {
            let response = self.get_cached(&request, robots).await?;

            if !response.status().is_redirection() {
                return Ok(response);
            }

            let url = request.url().join(str::from_utf8(
                response
                    .headers()
                    .get("location")
                    .ok_or(HttpClientError::RedirectLocation)?
                    .as_bytes(),
            )?)?;
            request = request.set_url(url);
        }

        Err(HttpClientError::TooManyRedirects)
    }

    // TODO Configure rate limits.
    // TODO Configure retries.
    // TODO Configure maximum connections.
    async fn get_cached(
        &self,
        request: &Request,
        robots: bool,
    ) -> Result<Arc<Response>, HttpClientError> {
        let get = || {
            self.0.cache.get_with(request.url().to_string(), {
                let request = request.clone();
                let client = self.cloned();

                Box::new(async move {
                    if robots
                        && let Some(robot) = client.get_robot(&request).await?
                        && !robot.is_absolute_allowed(request.url())
                    {
                        return Err(HttpClientError::RobotsTxt);
                    }

                    let response = client.get_retried(&request).await?;

                    Ok(Arc::new(response.into()))
                })
            })
        };

        let response = get().await??;

        Ok(if let Some(age) = request.max_age()
            && response.is_expired(age)
        {
            self.0.cache.remove(request.url().as_str()).await?;

            get().await??
        } else {
            response
        }
        .response()
        .clone())
    }

    async fn get_retried(&self, request: &Request) -> Result<Response, HttpClientError> {
        let retry = request.retry();
        let mut result = self.get_throttled(request).await;
        let mut backoff = retry.duration().initial();

        for _ in 0..retry.count() {
            if let Ok(response) = &result
                && !response.status().is_server_error()
            {
                break;
            }

            sleep(backoff).await;
            backoff = backoff.mul_f64(retry.factor());

            if let Some(cap) = retry.duration().cap() {
                backoff = backoff.min(cap);
            }

            result = self.get_throttled(request).await;
        }

        result
    }

    async fn get_throttled(&self, request: &Request) -> Result<Response, HttpClientError> {
        let _permit = self.0.semaphore.acquire().await.unwrap();
        self.get_once(request).await
    }

    async fn get_once(&self, request: &Request) -> Result<Response, HttpClientError> {
        let start = self.0.timer.now();
        // TODO Use a custom timeout implementation that would be reliable on CI.
        let response = timeout(request.timeout(), self.0.client.get(request.as_bare())).await??;
        let duration = self.0.timer.now().duration_since(start);

        Ok(Response::from_bare(response, duration))
    }

    #[async_recursion]
    async fn get_robot(&self, request: &Request) -> Result<Option<Robots>, HttpClientError> {
        Ok(self
            .get_inner(
                &request.clone().set_url(request.url().join("/robots.txt")?),
                false,
            )
            .await
            .ok()
            .map(|response| Robots::from_bytes(response.body(), USER_AGENT)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cache::MemoryCache,
        http_client::{BareResponse, StubHttpClient, StubSequenceHttpClient, build_stub_response},
        timer::StubTimer,
    };
    use core::time::Duration;
    use http::{HeaderName, HeaderValue, StatusCode};
    use pretty_assertions::assert_eq;
    use url::Url;

    const CACHE_CAPACITY: usize = 1 << 16;

    #[test]
    fn build_client() {
        HttpClient::new(
            StubHttpClient::new(Default::default()),
            StubTimer::new(),
            Box::new(MemoryCache::new(0)),
            1,
        );
    }

    #[tokio::test]
    async fn get() {
        let response = BareResponse {
            url: Url::parse("https://foo.com").unwrap().clone(),
            status: StatusCode::OK,
            headers: Default::default(),
            body: vec![],
        };

        assert_eq!(
            HttpClient::new(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            response.url.join("/robots.txt").unwrap().as_str(),
                            StatusCode::OK,
                            Default::default(),
                            vec![],
                        ),
                        (response.url.as_str().into(), Ok(response.clone()))
                    ]
                    .into_iter()
                    .collect()
                ),
                StubTimer::new(),
                Box::new(MemoryCache::new(CACHE_CAPACITY)),
                1,
            )
            .get(&Request::new(response.url.clone(), Default::default()))
            .await
            .unwrap(),
            Some(Response::from_bare(response, Duration::from_millis(0)).into())
        );
    }

    #[tokio::test]
    async fn get_slash() {
        let response = BareResponse {
            url: Url::parse("https://foo.com/bar/").unwrap().clone(),
            status: StatusCode::OK,
            headers: Default::default(),
            body: vec![],
        };

        assert_eq!(
            HttpClient::new(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            response.url.join("/robots.txt").unwrap().as_str(),
                            StatusCode::OK,
                            Default::default(),
                            vec![],
                        ),
                        (response.url.as_str().into(), Ok(response.clone()))
                    ]
                    .into_iter()
                    .collect()
                ),
                StubTimer::new(),
                Box::new(MemoryCache::new(CACHE_CAPACITY)),
                1,
            )
            .get(&Request::new(response.url.clone(), Default::default()))
            .await
            .unwrap(),
            Some(Response::from_bare(response, Duration::from_millis(0)).into())
        );
    }

    #[tokio::test]
    async fn redirect() {
        let foo_response = BareResponse {
            url: Url::parse("https://foo.com").unwrap(),
            status: StatusCode::MOVED_PERMANENTLY,
            headers: [(
                HeaderName::from_static("location"),
                HeaderValue::from_static("https://bar.com"),
            )]
            .into_iter()
            .collect(),
            body: vec![],
        };
        let bar_response = BareResponse {
            url: Url::parse("https://bar.com").unwrap(),
            status: StatusCode::OK,
            ..foo_response.clone()
        };

        assert_eq!(
            HttpClient::new(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            foo_response.url.join("/robots.txt").unwrap().as_str(),
                            StatusCode::OK,
                            Default::default(),
                            vec![],
                        ),
                        build_stub_response(
                            bar_response.url.join("/robots.txt").unwrap().as_str(),
                            StatusCode::OK,
                            Default::default(),
                            vec![],
                        ),
                        (foo_response.url.clone().into(), Ok(foo_response.clone())),
                        (bar_response.url.clone().into(), Ok(bar_response.clone())),
                    ]
                    .into_iter()
                    .collect()
                ),
                StubTimer::new(),
                Box::new(MemoryCache::new(CACHE_CAPACITY)),
                1,
            )
            .get(&Request::new(foo_response.url.clone(), Default::default()).set_max_redirects(1))
            .await
            .unwrap(),
            Some(Response::from_bare(bar_response, Duration::from_millis(0)).into())
        );
    }

    #[tokio::test]
    async fn redirect_never() {
        let foo_response = BareResponse {
            url: Url::parse("https://foo.com").unwrap(),
            status: StatusCode::MOVED_PERMANENTLY,
            headers: [(
                HeaderName::from_static("location"),
                HeaderValue::from_static("https://bar.com"),
            )]
            .into_iter()
            .collect(),
            body: vec![],
        };
        let bar_response = BareResponse {
            url: Url::parse("https://bar.com").unwrap(),
            status: StatusCode::OK,
            ..foo_response.clone()
        };

        assert_eq!(
            HttpClient::new(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            foo_response.url.join("/robots.txt").unwrap().as_str(),
                            StatusCode::OK,
                            Default::default(),
                            vec![],
                        ),
                        build_stub_response(
                            bar_response.url.join("/robots.txt").unwrap().as_str(),
                            StatusCode::OK,
                            Default::default(),
                            vec![],
                        ),
                        (foo_response.url.clone().into(), Ok(foo_response.clone())),
                        (bar_response.url.clone().into(), Ok(bar_response.clone())),
                    ]
                    .into_iter()
                    .collect()
                ),
                StubTimer::new(),
                Box::new(MemoryCache::new(CACHE_CAPACITY)),
                1,
            )
            .get(&Request::new(foo_response.url.clone(), Default::default()))
            .await,
            Err(HttpClientError::TooManyRedirects)
        );
    }

    #[tokio::test]
    async fn get_cache() {
        let url = Url::parse("https://foo.com").unwrap();
        let response = BareResponse {
            url: url.clone(),
            status: StatusCode::OK,
            headers: Default::default(),
            body: vec![],
        };

        let cache = MemoryCache::new(CACHE_CAPACITY);

        cache
            .get_with(url.as_str().into(), {
                let response = response.clone();

                Box::new(async move {
                    Ok(Arc::new(
                        Response::from_bare(
                            BareResponse {
                                body: b"stale".to_vec(),
                                ..response
                            },
                            Duration::default(),
                        )
                        .into(),
                    ))
                })
            })
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            HttpClient::new(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            url.join("/robots.txt").unwrap().as_str(),
                            StatusCode::OK,
                            Default::default(),
                            vec![],
                        ),
                        (url.as_str().into(), Ok(response.clone()))
                    ]
                    .into_iter()
                    .collect()
                ),
                StubTimer::new(),
                Box::new(cache),
                1,
            )
            .get(&Request::new(url, Default::default()))
            .await
            .unwrap(),
            Some(
                Response::from_bare(
                    BareResponse {
                        body: b"stale".to_vec(),
                        ..response
                    },
                    Duration::from_millis(0)
                )
                .into()
            )
        );
    }

    #[tokio::test]
    async fn update_cache() {
        let url = Url::parse("https://foo.com").unwrap();
        let response = BareResponse {
            url: url.clone(),
            status: StatusCode::OK,
            headers: Default::default(),
            body: vec![],
        };

        let cache = MemoryCache::new(CACHE_CAPACITY);

        cache
            .get_with(url.as_str().into(), {
                let response = response.clone();

                Box::new(async move {
                    Ok(Arc::new(
                        Response::from_bare(
                            BareResponse {
                                body: b"stale".to_vec(),
                                ..response
                            },
                            Duration::default(),
                        )
                        .into(),
                    ))
                })
            })
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            HttpClient::new(
                StubHttpClient::new(
                    [
                        build_stub_response(
                            url.join("/robots.txt").unwrap().as_str(),
                            StatusCode::OK,
                            Default::default(),
                            vec![],
                        ),
                        (url.as_str().into(), Ok(response.clone()))
                    ]
                    .into_iter()
                    .collect()
                ),
                StubTimer::new(),
                Box::new(cache),
                1,
            )
            .get(&Request::new(url, Default::default()).set_max_age(Some(Duration::ZERO)))
            .await
            .unwrap(),
            Some(Response::from_bare(response, Duration::from_millis(0)).into())
        );
    }

    #[tokio::test]
    async fn timeout() {
        let url = Url::parse("https://foo.com").unwrap();
        let response = BareResponse {
            url: url.clone(),
            status: StatusCode::OK,
            headers: Default::default(),
            body: vec![],
        };

        // TODO Use a fake timer.
        let result = HttpClient::new(
            StubHttpClient::new(
                [
                    build_stub_response(
                        url.join("/robots.txt").unwrap().as_str(),
                        StatusCode::OK,
                        Default::default(),
                        vec![],
                    ),
                    (url.as_str().into(), Ok(response.clone())),
                ]
                .into_iter()
                .collect(),
            )
            .set_delay(Duration::from_millis(50)),
            StubTimer::new(),
            Box::new(MemoryCache::new(CACHE_CAPACITY)),
            1,
        )
        .get(&Request::new(url, Default::default()).set_timeout(Duration::from_millis(1).into()))
        .await;

        assert!(matches!(result, Err(HttpClientError::Timeout(_))));
    }

    mod retry {
        use crate::RetryConfig;

        use super::*;
        use pretty_assertions::assert_eq;

        #[tokio::test]
        async fn retry_once_with_http_error() {
            let url = Url::parse("https://foo.com").unwrap();
            let response = BareResponse {
                url: url.clone(),
                status: StatusCode::OK,
                headers: Default::default(),
                body: vec![],
            };

            assert_eq!(
                HttpClient::new(
                    StubSequenceHttpClient::new(vec![
                        build_stub_response(
                            url.join("/robots.txt").unwrap().as_str(),
                            StatusCode::OK,
                            Default::default(),
                            vec![],
                        ),
                        (
                            url.as_str().into(),
                            Ok(BareResponse {
                                url: url.clone(),
                                status: StatusCode::INTERNAL_SERVER_ERROR,
                                headers: Default::default(),
                                body: vec![],
                            })
                        ),
                        (url.as_str().into(), Ok(response.clone())),
                    ]),
                    StubTimer::new(),
                    Box::new(MemoryCache::new(CACHE_CAPACITY)),
                    1,
                )
                .get(
                    &Request::new(url, Default::default())
                        .set_retry(RetryConfig::default().set_count(1).into())
                )
                .await
                .unwrap(),
                Some(Response::from_bare(response, Duration::from_millis(0)).into())
            );
        }

        #[tokio::test]
        async fn retry_once_with_non_http_error() {
            let url = Url::parse("https://foo.com").unwrap();
            let response = BareResponse {
                url: url.clone(),
                status: StatusCode::OK,
                headers: Default::default(),
                body: vec![],
            };

            assert_eq!(
                HttpClient::new(
                    StubSequenceHttpClient::new(vec![
                        build_stub_response(
                            url.join("/robots.txt").unwrap().as_str(),
                            StatusCode::OK,
                            Default::default(),
                            vec![],
                        ),
                        (
                            url.as_str().into(),
                            Err(HttpClientError::Http("foo".into()))
                        ),
                        (url.as_str().into(), Ok(response.clone())),
                    ]),
                    StubTimer::new(),
                    Box::new(MemoryCache::new(CACHE_CAPACITY)),
                    1,
                )
                .get(
                    &Request::new(url, Default::default())
                        .set_retry(RetryConfig::default().set_count(1).into())
                )
                .await
                .unwrap(),
                Some(Response::from_bare(response, Duration::from_millis(0)).into())
            );
        }

        #[tokio::test]
        async fn retry_once_with_two_errors() {
            let url = Url::parse("https://foo.com").unwrap();
            let failed_response = BareResponse {
                url: url.clone(),
                status: StatusCode::INTERNAL_SERVER_ERROR,
                headers: Default::default(),
                body: vec![],
            };
            let successful_response = BareResponse {
                url: url.clone(),
                status: StatusCode::OK,
                headers: Default::default(),
                body: vec![],
            };

            assert_eq!(
                HttpClient::new(
                    StubSequenceHttpClient::new(vec![
                        build_stub_response(
                            url.join("/robots.txt").unwrap().as_str(),
                            StatusCode::OK,
                            Default::default(),
                            vec![],
                        ),
                        (url.as_str().into(), Ok(failed_response.clone())),
                        (url.as_str().into(), Ok(failed_response.clone())),
                        (url.as_str().into(), Ok(successful_response.clone())),
                    ]),
                    StubTimer::new(),
                    Box::new(MemoryCache::new(CACHE_CAPACITY)),
                    1,
                )
                .get(
                    &Request::new(url, Default::default())
                        .set_retry(RetryConfig::default().set_count(1).into())
                )
                .await
                .unwrap(),
                Some(Response::from_bare(failed_response, Duration::from_millis(0)).into())
            );
        }

        #[tokio::test]
        async fn retry_twice_with_two_errors() {
            let url = Url::parse("https://foo.com").unwrap();
            let failed_response = BareResponse {
                url: url.clone(),
                status: StatusCode::INTERNAL_SERVER_ERROR,
                headers: Default::default(),
                body: vec![],
            };
            let successful_response = BareResponse {
                url: url.clone(),
                status: StatusCode::OK,
                headers: Default::default(),
                body: vec![],
            };

            assert_eq!(
                HttpClient::new(
                    StubSequenceHttpClient::new(vec![
                        build_stub_response(
                            url.join("/robots.txt").unwrap().as_str(),
                            StatusCode::OK,
                            Default::default(),
                            vec![],
                        ),
                        (url.as_str().into(), Ok(failed_response.clone())),
                        (url.as_str().into(), Ok(failed_response.clone())),
                        (url.as_str().into(), Ok(successful_response.clone())),
                    ]),
                    StubTimer::new(),
                    Box::new(MemoryCache::new(CACHE_CAPACITY)),
                    1,
                )
                .get(
                    &Request::new(url, Default::default())
                        .set_retry(RetryConfig::default().set_count(2).into())
                )
                .await
                .unwrap(),
                Some(Response::from_bare(successful_response, Duration::from_millis(0)).into())
            );
        }
    }
}
