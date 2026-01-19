use crate::response::Response;
use alloc::sync::Arc;
use core::time::Duration;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Debug, Serialize, Deserialize)]
pub struct CachedResponse {
    response: Arc<Response>,
    timestamp: SystemTime,
}

impl CachedResponse {
    pub fn new(response: Response) -> Self {
        Self {
            response: response.into(),
            timestamp: SystemTime::now(),
        }
    }

    pub const fn response(&self) -> &Arc<Response> {
        &self.response
    }

    pub fn is_expired(&self, duration: Duration) -> bool {
        SystemTime::now() > self.timestamp + duration
    }
}

impl From<Response> for CachedResponse {
    fn from(response: Response) -> Self {
        Self::new(response)
    }
}
