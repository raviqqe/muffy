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
