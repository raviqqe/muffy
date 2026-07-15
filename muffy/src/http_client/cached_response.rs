use super::HttpClientError;
use crate::response::Response;
use alloc::sync::Arc;
use core::time::Duration;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Debug, Serialize, Deserialize)]
pub struct CachedResponse {
    result: Result<Arc<Response>, HttpClientError>,
    timestamp: SystemTime,
}

impl CachedResponse {
    pub fn new(result: Result<Arc<Response>, HttpClientError>) -> Self {
        Self {
            result,
            timestamp: SystemTime::now(),
        }
    }

    pub const fn result(&self) -> &Result<Arc<Response>, HttpClientError> {
        &self.result
    }

    pub fn is_expired(&self, duration: Duration) -> bool {
        SystemTime::now() > self.timestamp + duration
    }
}
