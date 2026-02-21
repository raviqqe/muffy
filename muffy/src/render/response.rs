use crate::response::Response;
use http::StatusCode;
use serde::Serialize;
use url::Url;

#[derive(Debug, Serialize)]
pub struct RenderedResponse<'a> {
    url: &'a Url,
    #[serde(with = "http_serde::status_code")]
    status: StatusCode,
    latency: u128,
}

impl<'a> RenderedResponse<'a> {
    pub const fn url(&self) -> &'a Url {
        self.url
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
            url: response.url(),
            status: response.status(),
            latency: response.duration().as_millis(),
        }
    }
}
