use crate::response::Response;
use http::StatusCode;
use serde::Serialize;
use url::Url;

#[derive(Debug, Serialize)]
pub struct RenderedResponse<'a> {
    url: &'a Url,
    #[serde(with = "http_serde::status_code")]
    status: StatusCode,
    duration: u128,
}

impl<'a> RenderedResponse<'a> {
    pub fn url(&self) -> &'a Url {
        self.url
    }

    pub fn status(&self) -> StatusCode {
        self.status
    }

    pub fn duration(&self) -> u128 {
        self.duration
    }
}

impl<'a> From<&'a Response> for RenderedResponse<'a> {
    fn from(response: &'a Response) -> Self {
        Self {
            url: response.url(),
            status: response.status(),
            duration: response.duration().as_millis(),
        }
    }
}
