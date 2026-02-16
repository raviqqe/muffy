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
use crate::{
    ConcurrencyConfig, MokaCache, cache::Cache, default_concurrency, rate_limiter::RateLimiter,
    request::Request, response::Response, timer::Timer,
};
use alloc::sync::Arc;
use async_recursion::async_recursion;
use cached_response::CachedResponse;
use core::{str, time::Duration};
use robotxt::Robots;
use std::collections::HashMap;
use tokio::{
    sync::Semaphore,
    time::{sleep, timeout},
};

const USER_AGENT: &str = "muffy";
const INITIAL_CACHE_CAPACITY: usize = 1 << 8;

/// A full-featured HTTP client.
pub struct HttpClient {
    client: Box<dyn BareHttpClient>,
    timer: Box<dyn Timer>,
    local_cache: MokaCache<Result<Arc<Response>, HttpClientError>>,
    global_cache: Box<dyn Cache<Result<Arc<CachedResponse>, HttpClientError>>>,
    semaphore: Semaphore,
    site_semaphores: HashMap<String, Semaphore>,
    rate_limiter: Option<RateLimiter>,
}

impl HttpClient {
    /// Creates an HTTP client.
    pub fn new(
        client: impl BareHttpClient + 'static,
        timer: impl Timer + 'static,
        cache: Box<dyn Cache<Result<Arc<CachedResponse>, HttpClientError>>>,
        concurrency: &ConcurrencyConfig,
        rate_limiter: Option<RateLimiter>,
    ) -> Self {
        Self {
            client: Box::new(client),
            timer: Box::new(timer),
            local_cache: MokaCache::new(INITIAL_CACHE_CAPACITY),
            global_cache: cache,
            semaphore: Semaphore::new(concurrency.global().unwrap_or_else(default_concurrency)),
            site_semaphores: concurrency
                .sites()
                .iter()
                .map(|(key, &value)| (key.to_owned(), Semaphore::new(value)))
                .collect(),
            rate_limiter,
        }
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
            let response = self.get_cached_locally(&request, robots).await?;

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

    async fn get_cached_locally(
        &self,
        request: &Request,
        robots: bool,
    ) -> Result<Arc<Response>, HttpClientError> {
        self.local_cache
            .get_with(
                request.url().to_string(),
                Box::new(async move { self.get_cached_globally(&request, robots).await }),
            )
            .await?
    }

    // TODO Configure rate limits.
    async fn get_cached_globally(
        &self,
        request: &Request,
        robots: bool,
    ) -> Result<Arc<Response>, HttpClientError> {
        let get = || {
            self.global_cache.get_with(
                request.url().to_string(),
                Box::new(async move {
                    if robots
                        && let Some(robot) = self.get_robot(&request).await?
                        && !robot.is_absolute_allowed(request.url())
                    {
                        return Err(HttpClientError::RobotsTxt);
                    }

                    let response = self.get_retried(&request).await?;

                    Ok(Arc::new(response.into()))
                }),
            )
        };

        let response = get().await??;

        Ok(if response.is_expired(request.max_age()) {
            self.global_cache.remove(request.url().as_str()).await?;

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

            backoff = backoff
                .mul_f64(retry.factor())
                .min(retry.duration().cap().unwrap_or(Duration::MAX));

            result = self.get_throttled(request).await;
        }

        result
    }

    async fn get_throttled(&self, request: &Request) -> Result<Response, HttpClientError> {
        let _global = self.semaphore.acquire().await.unwrap();
        let _site = if let Some(id) = request.site_id()
            && let Some(semaphore) = self.site_semaphores.get(id)
        {
            Some(semaphore.acquire().await.unwrap())
        } else {
            None
        };

        let future = self.get_once(request);

        if let Some(limiter) = &self.rate_limiter {
            limiter.run(future).await
        } else {
            future.await
        }
    }

    async fn get_once(&self, request: &Request) -> Result<Response, HttpClientError> {
        let start = self.timer.now();
        // TODO Use a custom timeout implementation that would be reliable on CI.
        let response = timeout(request.timeout(), self.client.get(request.as_bare())).await??;
        let duration = self.timer.now().duration_since(start);

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
    use crate::RetryConfig;
    use crate::{
        ConcurrencyConfig,
        cache::MemoryCache,
        http_client::{BareResponse, StubHttpClient, StubSequenceHttpClient, build_stub_response},
        timer::StubTimer,
    };
    use core::time::Duration;
    use http::{HeaderName, HeaderValue, StatusCode};
    use pretty_assertions::assert_eq;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use tokio::spawn;
    use url::Url;

    const CACHE_CAPACITY: usize = 1 << 16;
    const CACHE_MAX_AGE: Duration = Duration::from_hours(1);

    #[test]
    fn build_client() {
        HttpClient::new(
            StubHttpClient::new(Default::default()),
            StubTimer::new(),
            Box::new(MemoryCache::new(0)),
            &Default::default(),
            None,
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
                &Default::default(),
                None,
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
                &Default::default(),
                None,
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
                &Default::default(),
                None,
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
                &Default::default(),
                None,
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
                &Default::default(),
                None,
            )
            .get(&Request::new(url, Default::default()).set_max_age(CACHE_MAX_AGE))
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
                &Default::default(),
                None,
            )
            .get(&Request::new(url, Default::default()))
            .await
            .unwrap(),
            Some(Response::from_bare(response, Duration::from_millis(0)).into())
        );
    }

    #[tokio::test]
    async fn hit_timeout() {
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
            &Default::default(),
            None,
        )
        .get(&Request::new(url, Default::default()).set_timeout(Duration::from_millis(1).into()))
        .await;

        assert!(matches!(result, Err(HttpClientError::Timeout(_))));
    }

    mod concurrency {
        use super::*;
        use async_trait::async_trait;
        use pretty_assertions::assert_eq;
        use tokio::sync::{Notify, mpsc};

        const CONCURRENT_REQUEST_DELAY: Duration = Duration::from_millis(50);

        struct FakeBareHttpClient {
            started: mpsc::UnboundedSender<()>,
            notify: Arc<Notify>,
            in_flight: Arc<AtomicUsize>,
            max_in_flight: Arc<AtomicUsize>,
        }

        fn send_request<'a>(
            client: &Arc<HttpClient>,
            request: Request,
        ) -> impl Future<Output = Result<Result<Response, HttpClientError>, tokio::task::JoinError>> + 'a
        {
            let client = client.clone();

            spawn(async move { client.get_throttled(&request).await })
        }

        #[async_trait]
        impl BareHttpClient for FakeBareHttpClient {
            async fn get(&self, request: &BareRequest) -> Result<BareResponse, HttpClientError> {
                let in_flight = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;

                self.max_in_flight.fetch_max(in_flight, Ordering::SeqCst);
                self.started.send(()).unwrap();
                self.notify.notified().await;

                self.in_flight.fetch_sub(1, Ordering::SeqCst);

                Ok(BareResponse {
                    url: request.url.clone(),
                    status: StatusCode::OK,
                    headers: Default::default(),
                    body: Default::default(),
                })
            }
        }

        #[tokio::test]
        async fn limit_concurrency_of_site() {
            let (sender, _receiver) = mpsc::unbounded_channel();
            let notify = Arc::new(Notify::new());
            let max_in_flight = Arc::new(AtomicUsize::new(0));

            let client = HttpClient::new(
                FakeBareHttpClient {
                    started: sender,
                    notify: notify.clone(),
                    in_flight: Default::default(),
                    max_in_flight: max_in_flight.clone(),
                },
                StubTimer::new(),
                Box::new(MemoryCache::new(CACHE_CAPACITY)),
                &ConcurrencyConfig::default()
                    .set_global(Some(2))
                    .set_sites([("foo".to_string(), 1)].into()),
                None,
            )
            .into();

            let request1 =
                Request::new(Url::parse("https://foo.com/").unwrap(), Default::default())
                    .set_site_id(Some("foo".into()));
            let request2 = request1
                .clone()
                .set_url(Url::parse("https://foo.com/bar").unwrap());

            let handle1 = send_request(&client, request1);
            let handle2 = send_request(&client, request2);

            sleep(CONCURRENT_REQUEST_DELAY).await;
            notify.notify_one();
            notify.notify_one();

            handle1.await.unwrap().unwrap();
            handle2.await.unwrap().unwrap();

            assert_eq!(max_in_flight.load(Ordering::SeqCst), 1);
        }

        #[tokio::test]
        async fn limit_concurrency_of_two_sites() {
            let (sender, mut receiver) = mpsc::unbounded_channel();
            let notify = Arc::new(Notify::new());
            let in_flight = Arc::new(AtomicUsize::new(0));
            let max_in_flight = Arc::new(AtomicUsize::new(0));

            let bare = FakeBareHttpClient {
                started: sender,
                notify: notify.clone(),
                in_flight: in_flight.clone(),
                max_in_flight: max_in_flight.clone(),
            };

            let concurrency = ConcurrencyConfig::default()
                .set_global(Some(2))
                .set_sites([("foo".to_string(), 1), ("bar".to_string(), 1)].into());
            let client = HttpClient::new(
                bare,
                StubTimer::new(),
                Box::new(MemoryCache::new(CACHE_CAPACITY)),
                &concurrency,
                None,
            )
            .into();

            let request1 =
                Request::new(Url::parse("https://foo.com/").unwrap(), Default::default())
                    .set_site_id(Some("foo".into()));
            let request2 =
                Request::new(Url::parse("https://bar.com/").unwrap(), Default::default())
                    .set_site_id(Some("bar".into()));

            let handle1 = send_request(&client, request1);
            let handle2 = send_request(&client, request2);

            receiver.recv().await.unwrap();
            receiver.recv().await.unwrap();

            assert_eq!(in_flight.load(Ordering::SeqCst), 2);
            assert_eq!(max_in_flight.load(Ordering::SeqCst), 2);

            notify.notify_one();
            notify.notify_one();

            handle1.await.unwrap().unwrap();
            handle2.await.unwrap().unwrap();
        }
    }

    mod retry {
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
                    &Default::default(),
                    None,
                )
                .get(
                    &Request::new(url, Default::default())
                        .set_max_age(CACHE_MAX_AGE)
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
                    &Default::default(),
                    None,
                )
                .get(
                    &Request::new(url, Default::default())
                        .set_max_age(CACHE_MAX_AGE)
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
                    &Default::default(),
                    None,
                )
                .get(
                    &Request::new(url, Default::default())
                        .set_max_age(CACHE_MAX_AGE)
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
                    &Default::default(),
                    None,
                )
                .get(
                    &Request::new(url, Default::default())
                        .set_max_age(CACHE_MAX_AGE)
                        .set_retry(RetryConfig::default().set_count(2).into())
                )
                .await
                .unwrap(),
                Some(Response::from_bare(successful_response, Duration::from_millis(0)).into())
            );
        }
    }
}
