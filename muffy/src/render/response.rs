use super::utility::abbreviate_url;
use crate::response::Response;
use alloc::borrow::Cow;
use http::StatusCode;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct RenderedResponse<'a> {
    url: Cow<'a, str>,
    #[serde(with = "http_serde::status_code")]
    status: StatusCode,
    latency: u128,
}

impl<'a> RenderedResponse<'a> {
    pub fn url(&self) -> &str {
        &self.url
    }

    pub const fn status(&self) -> StatusCode {
        self.status
    }

    pub const fn duration(&self) -> u128 {
        self.latency
    }
}

impl<'a> From<&'a Response> for RenderedResponse<'a> {
    fn from(response: &'a Response) -> Self {
        Self {
            url: abbreviate_url(response.url().as_str()),
            status: response.status(),
            latency: response.duration().as_millis(),
        }
    }
}
