use crate::{error::Error, http_client::HttpClient, response::Response};
use tokio::time::Instant;
use url::Url;

pub struct EnrichedHttpClient<T: HttpClient> {
    client: T,
}

impl<T: HttpClient> EnrichedHttpClient<T> {
    async fn get(&self, url: &Url) -> Result<Response, Error> {
        let start = Instant::now();
        let response = self.client.get(url).await?;
        let duration = Instant::now().duration_since(start);

        Ok(Response::from_bare(response, duration))
    }
}
