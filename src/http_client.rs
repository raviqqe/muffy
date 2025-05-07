mod bare;
mod error;
mod reqwest;
#[cfg(test)]
mod stub;

#[cfg(test)]
pub use self::stub::StubHttpClient;
pub use self::{
    bare::{BareHttpClient, BareResponse},
    error::HttpClientError,
    reqwest::ReqwestHttpClient,
};
use crate::{cache::Cache, response::Response, timer::Timer};
use alloc::sync::Arc;
use async_recursion::async_recursion;
use core::str;
use http::HeaderMap;
use robotxt::Robots;
use tokio::sync::Semaphore;
use url::Url;

const USER_AGENT: &str = "muffy";

/// A cached HTTP client.
pub struct HttpClient(Arc<HttpClientInner>);

struct HttpClientInner {
    client: Box<dyn BareHttpClient>,
    timer: Box<dyn Timer>,
    cache: Box<dyn Cache<Result<Arc<Response>, HttpClientError>>>,
    semaphore: Semaphore,
}

impl HttpClient {
    /// Creates an HTTP client.
    pub fn new(
        client: impl BareHttpClient + 'static,
        timer: impl Timer + 'static,
        cache: Box<dyn Cache<Result<Arc<Response>, HttpClientError>>>,
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
        url: &Url,
        headers: &HeaderMap,
    ) -> Result<Option<Arc<Response>>, HttpClientError> {
        match self.get_inner(url, headers, true).await {
            Ok(response) => Ok(Some(response)),
            Err(HttpClientError::RobotsTxt) => Ok(None),
            Err(error) => Err(error),
        }
    }

    async fn get_inner(
        &self,
        url: &Url,
        headers: &HeaderMap,
        robots: bool,
    ) -> Result<Arc<Response>, HttpClientError> {
        let mut url = url.clone();

        // TODO Configure maximum redirect counts.
        // TODO Configure rate limits.
        // TODO Configure timeouts.
        // TODO Configure maximum connections.
        loop {
            let response = self.get_once(&url, headers, robots).await?;

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
        &self,
        url: &Url,
        headers: &HeaderMap,
        robots: bool,
    ) -> Result<Arc<Response>, HttpClientError> {
        // TODO Configure cache expiry.
        self.0
            .cache
            .get_or_set(url.to_string(), {
                let url = url.clone();
                let headers = headers.clone();
                let client = self.cloned();

                Box::new(async move {
                    if robots {
                        if let Some(robot) = client.get_robot(&url, &headers).await? {
                            if !robot.is_absolute_allowed(&url) {
                                return Err(HttpClientError::RobotsTxt);
                            }
                        }
                    }

                    let permit = client.0.semaphore.acquire().await.unwrap();
                    let start = client.0.timer.now();
                    let response = client.0.client.get(&url, &headers).await?;
                    let duration = client.0.timer.now().duration_since(start);
                    drop(permit);

                    Ok(Response::from_bare(response, duration).into())
                })
            })
            .await?
    }

    #[async_recursion]
    async fn get_robot(
        &self,
        url: &Url,
        headers: &HeaderMap,
    ) -> Result<Option<Robots>, HttpClientError> {
        Ok(self
            .get_inner(&url.join("robots.txt")?, headers, false)
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
        http_client::{BareResponse, StubHttpClient},
        timer::StubTimer,
    };
    use core::time::Duration;
    use http::StatusCode;
    use pretty_assertions::assert_eq;

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
        let url = Url::parse("https://foo.com").unwrap();
        let robots_url = url.join("robots.txt").unwrap();
        let response = BareResponse {
            url: url.clone(),
            status: StatusCode::OK,
            headers: Default::default(),
            body: vec![],
        };

        assert_eq!(
            HttpClient::new(
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
            .get(&url, &Default::default())
            .await
            .unwrap(),
            Some(Response::from_bare(response, Duration::from_millis(0)).into())
        );
    }
}
