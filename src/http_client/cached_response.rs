use crate::response::Response;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize)]
pub struct CachedResponse {
    response: Response,
    timestamp: Duration,
}

impl CachedResponse {
    pub fn new(response: Response) -> Self {
        CachedResponse {
            response,
            timestamp: Self::now(),
        }
    }

    pub fn response(&self) -> &Response {
        &self.response
    }

    pub fn is_expired(&self, duration: Duration) -> bool {
        Self::now() - self.timestamp > duration
    }

    fn now() -> Duration {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap()
    }
}
