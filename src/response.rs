use reqwest::{StatusCode, header::HeaderMap};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Response {
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
    duration: Duration,
}

impl Response {
    pub const fn new(
        status: StatusCode,
        headers: HeaderMap,
        body: Vec<u8>,
        duration: Duration,
    ) -> Self {
        Self {
            status,
            headers,
            body,
            duration,
        }
    }

    pub const fn status(&self) -> StatusCode {
        self.status
    }

    pub const fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    pub const fn duration(&self) -> Duration {
        self.duration
    }
}
