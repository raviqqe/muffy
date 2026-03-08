use crate::{cache::CacheError, html_parser::HtmlParseError, http_client::HttpClientError};
use core::{
    error,
    fmt::{self, Display, Formatter},
    str::Utf8Error,
};
use http::StatusCode;
use serde::{Serialize, Serializer};
use std::io;
use tokio::{sync::AcquireError, task::JoinError};
use url::ParseError;

/// An error.
#[derive(Debug)]
pub enum Error {
    /// Semaphore acquirement failure.
    Acquire(AcquireError),
    /// A cache error.
    Cache(CacheError),
    /// An invalid content type.
    ContentTypeInvalid {
        /// An actual content type.
        actual: String,
        /// An expected content type.
        expected: &'static str,
    },
    /// An HTML parse error.
    HtmlParse(HtmlParseError),
    /// An HTTP client error.
    HttpClient(HttpClientError),
    /// An I/O error.
    Io(io::Error),
    /// An thread join error.
    Join(JoinError),
    /// A JSON serialization error.
    Json(serde_json::Error),
    /// A regular expression error.
    Regex(regex::Error),
    /// A sitemap error.
    Sitemap(sitemaps::error::Error),
    /// A Sled database error.
    Sled(sled::Error),
    /// A URL parse error.
    UrlParse(ParseError),
    /// A UTF-8 error.
    Utf8(Utf8Error),
    /// A validation failure.
    Validation,
}

impl error::Error for Error {}

impl Display for Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Acquire(error) => write!(formatter, "{error}"),
            Self::Cache(error) => write!(formatter, "{error}"),
            Self::ContentTypeInvalid { actual, expected } => {
                write!(
                    formatter,
                    "content type expected {expected} but got {actual}"
                )
            }
            Self::HtmlParse(error) => write!(formatter, "{error}"),
            Self::HttpClient(error) => write!(formatter, "{error}"),
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Join(error) => write!(formatter, "{error}"),
            Self::Json(error) => write!(formatter, "{error}"),
            Self::Regex(error) => write!(formatter, "{error}"),
            Self::Sitemap(error) => write!(formatter, "{error}"),
            Self::Sled(error) => write!(formatter, "{error}"),
            Self::UrlParse(error) => write!(formatter, "{error}"),
            Self::Utf8(error) => write!(formatter, "{error}"),
            Self::Validation => write!(formatter, "validation failed"),
        }
    }
}

/// An element item error.
#[derive(Debug)]
pub enum ItemError {
    /// An HTML element not found.
    HtmlElementNotFound(String),
    /// An HTML validation failure.
    HtmlValidation(muffy_validation::ValidationError),
    /// An HTTP client error.
    HttpClient(HttpClientError),
    /// An error status code in an HTTP response.
    HttpStatus(StatusCode),
    /// An invalid scheme.
    InvalidScheme(String),
}

impl error::Error for ItemError {}

impl Display for ItemError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::HtmlElementNotFound(name) => {
                write!(formatter, "HTML element for #{name} not found")
            }
            Self::HtmlValidation(error) => write!(formatter, "{error}"),
            Self::HttpClient(error) => write!(formatter, "{error}"),
            Self::HttpStatus(status) => write!(formatter, "invalid status {status}"),
            Self::InvalidScheme(scheme) => write!(formatter, "invalid scheme \"{scheme}\""),
        }
    }
}

impl Serialize for ItemError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<HttpClientError> for ItemError {
    fn from(error: HttpClientError) -> Self {
        Self::HttpClient(error)
    }
}

impl From<url::ParseError> for ItemError {
    fn from(error: url::ParseError) -> Self {
        Self::HttpClient(HttpClientError::UrlParse(error.to_string().into()))
    }
}

impl From<ItemError> for Error {
    fn from(error: ItemError) -> Self {
        match error {
            ItemError::HtmlElementNotFound(name) => Self::HttpClient(HttpClientError::Http(
                format!("element #{name} not found").into(),
            )),
            ItemError::HtmlValidation(error) => {
                Self::HttpClient(HttpClientError::Http(error.to_string().into()))
            }
            ItemError::HttpClient(error) => Self::HttpClient(error),
            ItemError::HttpStatus(status) => Self::HttpClient(HttpClientError::Http(
                format!("invalid status {status}").into(),
            )),
            ItemError::InvalidScheme(scheme) => Self::HttpClient(HttpClientError::Http(
                format!("invalid scheme \"{scheme}\"").into(),
            )),
        }
    }
}

impl From<AcquireError> for Error {
    fn from(error: AcquireError) -> Self {
        Self::Acquire(error)
    }
}

impl From<CacheError> for Error {
    fn from(error: CacheError) -> Self {
        Self::Cache(error)
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<HtmlParseError> for Error {
    fn from(error: HtmlParseError) -> Self {
        Self::HtmlParse(error)
    }
}

impl From<HttpClientError> for Error {
    fn from(error: HttpClientError) -> Self {
        Self::HttpClient(error)
    }
}

impl From<JoinError> for Error {
    fn from(error: JoinError) -> Self {
        Self::Join(error)
    }
}

impl From<regex::Error> for Error {
    fn from(error: regex::Error) -> Self {
        Self::Regex(error)
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<sled::Error> for Error {
    fn from(error: sled::Error) -> Self {
        Self::Sled(error)
    }
}

impl From<url::ParseError> for Error {
    fn from(error: url::ParseError) -> Self {
        Self::UrlParse(error)
    }
}

impl From<sitemaps::error::Error> for Error {
    fn from(error: sitemaps::error::Error) -> Self {
        Self::Sitemap(error)
    }
}

impl From<Utf8Error> for Error {
    fn from(error: Utf8Error) -> Self {
        Self::Utf8(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_item_html_validation_error() {
        assert_eq!(
            format!(
                "{}",
                ItemError::HtmlValidation(muffy_validation::ValidationError::UnknownTag(
                    "foo".into()
                ))
            ),
            "unknown tag \"foo\""
        );
    }
}
