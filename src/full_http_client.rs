use crate::{
    cache::Cache,
    error::Error,
    http_client::{HttpClient, HttpClientError},
    response::Response,
};
use alloc::sync::Arc;
use core::str;
use tokio::{sync::Semaphore, time::Instant};
use url::Url;

const CACHE_CAPACITY: usize = 1 << 16;

pub struct FullHttpClient {
    client: Box<dyn HttpClient>,
    cache: Cache<Result<Arc<Response>, HttpClientError>>,
    semaphore: Semaphore,
}

impl FullHttpClient {
    pub fn new(client: impl HttpClient + 'static, concurrency: usize) -> Self {
        Self {
            client: Box::new(client),
            cache: Cache::new(CACHE_CAPACITY),
            semaphore: Semaphore::new(concurrency),
        }
    }

    pub async fn get(&self, url: &Url) -> Result<Arc<Response>, Error> {
        let mut url = url.clone();
        let mut response;

        while {
            response = self.get_single(&url).await?;

            response.status().is_redirection()
        } {
            url = url.join(str::from_utf8(
                &response
                    .headers()
                    .get("location")
                    .ok_or_else(|| Error::RedirectLocation)?
                    .as_bytes(),
            )?)?;
        }

        Ok(response)
    }

    async fn get_single(&self, url: &Url) -> Result<Arc<Response>, Error> {
        Ok(self
            .cache
            .get_or_set(url.to_string(), async {
                let permit = self.semaphore.acquire().await.unwrap();
                let start = Instant::now();
                // TODO Redirect manually and cache intermediate responses.
                let response = self.client.get(url).await?;
                let duration = Instant::now().duration_since(start);
                drop(permit);

                Ok(Response::from_bare(response, duration).into())
            })
            .await?)
    }
}
