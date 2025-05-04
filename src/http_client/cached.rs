use crate::{
    cache::Cache,
    http_client::{HttpClient, HttpClientError},
    response::Response,
    timer::Timer,
};
use alloc::sync::Arc;
use async_recursion::async_recursion;
use core::str;
use robotxt::Robots;
use tokio::sync::Semaphore;
use url::Url;

const USER_AGENT: &str = "muffy";

pub struct CachedHttpClient(Arc<CachedHttpClientInner>);

struct CachedHttpClientInner {
    client: Box<dyn HttpClient>,
    timer: Box<dyn Timer>,
    cache: Box<dyn Cache<Result<Arc<Response>, HttpClientError>>>,
    semaphore: Semaphore,
}

impl CachedHttpClient {
    pub fn new(
        client: impl HttpClient + 'static,
        timer: impl Timer + 'static,
        cache: Box<dyn Cache<Result<Arc<Response>, HttpClientError>>>,
        concurrency: usize,
    ) -> Self {
        Self(
            CachedHttpClientInner {
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

    pub async fn get(&self, url: &Url) -> Result<Option<Arc<Response>>, HttpClientError> {
        match self.get_inner(url, true).await {
            Ok(response) => Ok(Some(response)),
            Err(HttpClientError::RobotsTxt) => Ok(None),
            Err(error) => Err(error),
        }
    }

    async fn get_inner(&self, url: &Url, robots: bool) -> Result<Arc<Response>, HttpClientError> {
        let mut url = url.clone();

        // TODO Configure maximum redirect counts.
        // TODO Configure rate limits.
        // TODO Configure timeouts.
        // TODO Configure maximum connections.
        loop {
            let response = self.get_once(&url, robots).await?;

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

    async fn get_once(&self, url: &Url, robots: bool) -> Result<Arc<Response>, HttpClientError> {
        // TODO Configure cache expiry.
        self.0
            .cache
            .get_or_set(url.to_string(), {
                let url = url.clone();
                let this = self.cloned();

                Box::new(async move {
                    if robots {
                        if let Some(robot) = this.get_robot(&url).await? {
                            if !robot.is_absolute_allowed(&url) {
                                return Err(HttpClientError::RobotsTxt);
                            }
                        }
                    }

                    let permit = this.0.semaphore.acquire().await.unwrap();
                    let start = this.0.timer.now();
                    let response = this.0.client.get(&url).await?;
                    let duration = this.0.timer.now().duration_since(start);
                    drop(permit);

                    Ok(Response::from_bare(response, duration).into())
                })
            })
            .await?
    }

    #[async_recursion]
    async fn get_robot(&self, url: &Url) -> Result<Option<Robots>, HttpClientError> {
        Ok(self
            .get_inner(&url.join("robots.txt")?, false)
            .await
            .ok()
            .map(|response| Robots::from_bytes(response.body(), USER_AGENT)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cache::MemoryCache, http_client::BareResponse, http_client::StubHttpClient,
        timer::StubTimer,
    };
    use core::time::Duration;
    use http::StatusCode;
    use pretty_assertions::assert_eq;

    const CACHE_CAPACITY: usize = 1 << 16;

    #[test]
    fn build_client() {
        CachedHttpClient::new(
            StubHttpClient::new(Default::default()),
            StubTimer::new(),
            Box::new(MemoryCache::new(0)),
            1,
        );
    }

    #[tokio::test]
    async fn get() {
        let url = Url::parse("https://foo.com").unwrap();
        let robots_url = url.join("robots.txt").unwrap();
        let response = BareResponse {
            url: url.clone(),
            status: StatusCode::OK,
            headers: Default::default(),
            body: vec![],
        };

        assert_eq!(
            CachedHttpClient::new(
                StubHttpClient::new(
                    [
                        (
                            robots_url.as_str().into(),
                            Ok(BareResponse {
                                url: robots_url,
                                status: StatusCode::OK,
                                headers: Default::default(),
                                body: vec![],
                            })
                        ),
                        (url.as_str().into(), Ok(response.clone()))
                    ]
                    .into_iter()
                    .collect()
                ),
                StubTimer::new(),
                Box::new(MemoryCache::new(CACHE_CAPACITY)),
                1,
            )
            .get(&url)
            .await
            .unwrap(),
            Some(Response::from_bare(response, Duration::from_millis(0)).into())
        );
    }
}
