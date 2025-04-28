use crate::{
    cache::Cache,
    error::Error,
    http_client::{HttpClient, HttpClientError},
    response::Response,
};
use alloc::sync::Arc;
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
    robots: Box<dyn Cache<Result<Robots, HttpClientError>>>,
}

impl FullHttpClient {
    pub fn new(
        client: impl HttpClient + 'static,
        cache: Box<dyn Cache<Result<Arc<Response>, HttpClientError>>>,
        robots: Box<dyn Cache<Result<Robots, HttpClientError>>>,
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

    pub async fn get(&self, url: &Url) -> Result<Arc<Response>, Error> {
        let mut url = url.clone();

        // TODO Configure maximum redirect counts.
        // TODO Configure rate limits.
        // TODO Configure timeouts.
        // TODO Configure maximum connections.
        loop {
            let response = self.get_once(&url).await?;

            if !response.status().is_redirection() {
                return Ok(response);
            }

            url = url.join(str::from_utf8(
                response
                    .headers()
                    .get("location")
                    .ok_or_else(|| Error::RedirectLocation)?
                    .as_bytes(),
            )?)?;
        }
    }

    async fn get_once(&self, url: &Url) -> Result<Arc<Response>, Error> {
        Ok(self
            .0
            .cache
            .get_or_set(url.to_string(), {
                let url = url.clone();
                let inner = self.0.clone();

                Box::new(async move {
                    let robot = Self::get_robot(&inner, &url).await?;

                    if !robot.is_absolute_allowed(&url) {
                        return Err(HttpClientError::RobotsTxt);
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
        Ok(inner
            .robots
            .get_or_set(
                url.host()
                    .ok_or_else(|| HttpClientError::HostNotDefined)?
                    .to_string(),
                {
                    let url = url.clone();
                    let inner = inner.clone();

                    Box::new(async move {
                        let response = inner.client.get(&url.join("robots.txt")?).await?;

                        Ok(Robots::from_bytes(&response.body, USER_AGENT))
                    })
                },
            )
            .await??)
    }
}
