use crate::response::Response;
use alloc::sync::Arc;
use core::time::Duration;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Serialize, Deserialize)]
pub struct CachedResponse {
    response: Arc<Response>,
    timestamp: Duration,
}

impl CachedResponse {
    pub fn new(response: Response) -> Self {
        Self {
            response: response.into(),
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("positive duration"),
        }
    }

    pub const fn response(&self) -> &Arc<Response> {
        &self.response
    }

    pub fn is_expired(&self, expiry: SystemTime) -> bool {
        UNIX_EPOCH + self.timestamp < expiry
    }
}

impl From<Response> for CachedResponse {
    fn from(response: Response) -> Self {
        Self::new(response)
    }
}
