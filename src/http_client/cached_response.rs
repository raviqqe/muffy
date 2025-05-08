use crate::response::Response;
use serde::{Deserialize, Serialize};
use tokio::time::Instant;

#[derive(Debug, Serialize, Deserialize)]
pub struct CachedResponse {
    response: Response,
    timestamp: Instant,
}

impl CachedResponse {
    pub fn new(response: Response) -> Self {
        CachedResponse {
            response,
            timestamp: Instant::now(),
        }
    }
    pub fn is_expired(&self, max_age: Duration) -> bool {
        self.timestamp.elapsed() > max_age
    }

    pub fn response(&self) -> &Response {
        &self.response
    }
}
