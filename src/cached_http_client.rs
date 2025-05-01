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

const USER_AGENT: &str = "muffy";

pub struct CachedHttpClient(Arc<CachedHttpClientInner>);

struct CachedHttpClientInner {
    client: Box<dyn HttpClient>,
    cache: Box<dyn Cache<Result<Arc<Response>, HttpClientError>>>,
    semaphore: Semaphore,
}

impl CachedHttpClient {
    pub fn new(
        client: impl HttpClient + 'static,
        cache: Box<dyn Cache<Result<Arc<Response>, HttpClientError>>>,
        concurrency: usize,
    ) -> Self {
        Self(
            CachedHttpClientInner {
                client: Box::new(client),
                cache,
                semaphore: Semaphore::new(concurrency),
            }
            .into(),
        )
    }

    pub async fn get(&self, url: &Url) -> Result<Arc<Response>, HttpClientError> {
        Self::get_inner(&self.0, url, true).await
    }

    async fn get_inner(
        inner: &Arc<CachedHttpClientInner>,
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
                    .ok_or(HttpClientError::RedirectLocation)?
                    .as_bytes(),
            )?)?;
        }
    }

    async fn get_once(
        inner: &Arc<CachedHttpClientInner>,
        url: &Url,
        robots: bool,
    ) -> Result<Arc<Response>, HttpClientError> {
        // TODO Configure cache expiry.
        inner
            .cache
            .get_or_set(url.to_string(), {
                let url = url.clone();
                let inner = inner.clone();

                Box::new(async move {
                    if robots {
                        if let Some(robot) = Self::get_robot(&inner, &url).await? {
                            if !robot.is_absolute_allowed(&url) {
                                return Err(HttpClientError::RobotsTxt);
                            }
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
            .await?
    }

    #[async_recursion]
    async fn get_robot(
        inner: &Arc<CachedHttpClientInner>,
        url: &Url,
    ) -> Result<Option<Robots>, HttpClientError> {
        Ok(Self::get_inner(inner, &url.join("robots.txt")?, false)
            .await
            .ok()
            .map(|response| Robots::from_bytes(response.body(), USER_AGENT)))
    }
}

#[cfg(test)]
mod tests {
    use crate::test::StubHttpClient;

    use super::*;

    #[tokio::test]
    async fn build_client() {
        CachedHttpClient::new(StubHttpClient::new(vec![]), 1, 1);
    }
}
