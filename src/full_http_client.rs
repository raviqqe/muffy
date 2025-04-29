use crate::{
    cache::Cache,
    http_client::{HttpClient, HttpClientError},
    response::Response,
};
use alloc::sync::Arc;
use async_recursion::async_recursion;
use core::str;
use robotxt::Robots;
use tokio::{sync::Semaphore, time::Instant};
use url::Url;

const USER_AGENT: &str = "muffin";

pub struct FullHttpClient(Arc<FullHttpClientInner>);

struct FullHttpClientInner {
    client: Box<dyn HttpClient>,
    cache: Box<dyn Cache<Result<Arc<Response>, HttpClientError>>>,
    semaphore: Semaphore,
    robots: Box<dyn Cache<Result<Arc<Response>, HttpClientError>>>,
}

impl FullHttpClient {
    pub fn new(
        client: impl HttpClient + 'static,
        cache: Box<dyn Cache<Result<Arc<Response>, HttpClientError>>>,
        robots: Box<dyn Cache<Result<Arc<Response>, HttpClientError>>>,
        concurrency: usize,
    ) -> Self {
        Self(
            FullHttpClientInner {
                client: Box::new(client),
                cache,
                robots,
                semaphore: Semaphore::new(concurrency),
            }
            .into(),
        )
    }

    pub async fn get(&self, url: &Url) -> Result<Arc<Response>, HttpClientError> {
        Self::get_inner(&self.0, url, true).await
    }

    async fn get_inner(
        inner: &Arc<FullHttpClientInner>,
        url: &Url,
        robots: bool,
    ) -> Result<Arc<Response>, HttpClientError> {
        let mut url = url.clone();

        // TODO Configure maximum redirect counts.
        // TODO Configure rate limits.
        // TODO Configure timeouts.
        // TODO Configure maximum connections.
        loop {
            let response = Self::get_once(inner, &url, robots).await?;

            if !response.status().is_redirection() {
                return Ok(response);
            }

            url = url.join(str::from_utf8(
                response
                    .headers()
                    .get("location")
                    .ok_or_else(|| HttpClientError::RedirectLocation)?
                    .as_bytes(),
            )?)?;
        }
    }

    #[async_recursion]
    async fn get_once(
        inner: &Arc<FullHttpClientInner>,
        url: &Url,
        robots: bool,
    ) -> Result<Arc<Response>, HttpClientError> {
        Ok(inner
            .cache
            .get_or_set(url.to_string(), {
                let url = url.clone();
                let inner = inner.clone();

                Box::new(async move {
                    if robots {
                        let robot = Self::get_robot(&inner, &url).await?;

                        if !robot.is_absolute_allowed(&url) {
                            return Err(HttpClientError::RobotsTxt);
                        }
                    }

                    let permit = inner.semaphore.acquire().await.unwrap();
                    let start = Instant::now();
                    let response = inner.client.get(&url).await?;
                    let duration = Instant::now().duration_since(start);
                    drop(permit);

                    Ok(Response::from_bare(response, duration).into())
                })
            })
            .await??)
    }

    async fn get_robot(
        inner: &Arc<FullHttpClientInner>,
        url: &Url,
    ) -> Result<Robots, HttpClientError> {
        Ok(Robots::from_bytes(
            inner
                .robots
                .get_or_set(
                    url.host()
                        .ok_or(HttpClientError::HostNotDefined)?
                        .to_string(),
                    {
                        let url = url.clone();
                        let inner = inner.clone();

                        Box::new(async move {
                            Self::get_inner(&inner, &url.join("robots.txt")?, false).await
                        })
                    },
                )
                .await??
                .body(),
            USER_AGENT,
        ))
    }
}
