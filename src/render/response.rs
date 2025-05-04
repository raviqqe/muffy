use crate::response::Response;
use http::StatusCode;
use serde::Serialize;
use url::Url;

#[derive(Serialize)]
pub struct RenderedResponse<'a> {
    url: &'a Url,
    #[serde(with = "http_serde::status_code")]
    status: StatusCode,
    duration: u128,
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
