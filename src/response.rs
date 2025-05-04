use crate::http_client::BareResponse;
use core::time::Duration;
use http::{StatusCode, header::HeaderMap};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Response {
    url: Url,
    #[serde(with = "http_serde::status_code")]
    status: StatusCode,
    #[serde(with = "http_serde::header_map")]
    headers: HeaderMap,
    body: Vec<u8>,
    duration: Duration,
}

impl Response {
    pub const fn new(
        url: Url,
        status: StatusCode,
        headers: HeaderMap,
        body: Vec<u8>,
        duration: Duration,
    ) -> Self {
        Self {
            url,
            status,
            headers,
            body,
            duration,
        }
    }

    pub fn from_bare(response: BareResponse, duration: Duration) -> Self {
        Self::new(
            response.url,
            response.status,
            response.headers,
            response.body,
            duration,
        )
    }

    pub const fn url(&self) -> &Url {
        &self.url
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

// TODO Move this under the `render` module.
#[derive(Serialize)]
pub(crate) struct SerializedResponse {
    url: Url,
    #[serde(with = "http_serde::status_code")]
    status: StatusCode,
    duration: u128,
}

impl SerializedResponse {
    pub(crate) fn from_response(response: &Response) -> Self {
        Self {
            url: response.url.clone(),
            status: response.status,
            duration: response.duration.as_millis(),
        }
    }
}
