use crate::http_client::BareResponse;
use core::{
    str::{self, Utf8Error},
    time::Duration,
};
use http::{
    StatusCode,
    header::{CONTENT_TYPE, HeaderMap},
};
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

    pub fn media_type(&self) -> Result<Option<&str>, Utf8Error> {
        self.headers
            .get(CONTENT_TYPE)
            .map(|value| {
                Ok(str::from_utf8(
                    value
                        .as_bytes()
                        .split(|byte| *byte == b';')
                        .next()
                        .unwrap_or_default(),
                )?
                .trim())
            })
            .transpose()
    }

    #[allow(clippy::missing_const_for_fn)]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    pub const fn duration(&self) -> Duration {
        self.duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http::HeaderValue;
    use pretty_assertions::assert_eq;

    fn create_response(headers: HeaderMap) -> Response {
        Response::new(
            Url::parse("https://foo.com").unwrap(),
            StatusCode::OK,
            headers,
            Default::default(),
            Default::default(),
        )
    }

    #[test]
    fn parse_no_media_type() {
        assert_eq!(create_response(Default::default()).media_type(), Ok(None));
    }

    #[test]
    fn parse_media_type() {
        assert_eq!(
            create_response(HeaderMap::from_iter([(
                CONTENT_TYPE,
                HeaderValue::from_static("text/html")
            )]))
            .media_type(),
            Ok(Some("text/html"))
        );
    }

    #[test]
    fn trim_media_type_with_parameter() {
        assert_eq!(
            create_response(HeaderMap::from_iter([(
                CONTENT_TYPE,
                HeaderValue::from_static(" text/html ; charset=utf-8")
            )]))
            .media_type(),
            Ok(Some("text/html"))
        );
    }

    #[test]
    fn keep_media_type_case() {
        assert_eq!(
            create_response(HeaderMap::from_iter([(
                CONTENT_TYPE,
                HeaderValue::from_static("Image/SVG+XML")
            )]))
            .media_type(),
            Ok(Some("Image/SVG+XML"))
        );
    }

    #[test]
    fn parse_media_type_with_non_ascii_parameter() {
        assert_eq!(
            create_response(HeaderMap::from_iter([(
                CONTENT_TYPE,
                HeaderValue::from_bytes(b"image/svg+xml; note=caf\xE9").unwrap()
            )]))
            .media_type(),
            Ok(Some("image/svg+xml"))
        );
    }

    #[test]
    fn fail_on_invalid_media_type_encoding() {
        assert!(
            create_response(HeaderMap::from_iter([(
                CONTENT_TYPE,
                HeaderValue::from_bytes(b"image/\xFFsvg").unwrap()
            )]))
            .media_type()
            .is_err()
        );
    }
}
