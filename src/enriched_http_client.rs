use crate::{context::Context, error::Error, http_client::HttpClient, response::Response};
use tokio::time::Instant;
use url::Url;

pub struct EnrichedHttpClient<T: HttpClient> {
    client: T,
}

impl<T: HttpClient> EnrichedHttpClient<T> {
    pub fn new(client: T) -> Self {
        Self { client }
    }

    pub async fn get(&self, context: &Context<T>, url: &Url) -> Result<Response, Error> {
        let permit = context.file_semaphore().acquire().await.unwrap();
        let start = Instant::now();
        let response = self.client.get(url).await?;
        let duration = Instant::now().duration_since(start);
        drop(permit);

        Ok(Response::from_bare(response, duration))
    }
}
