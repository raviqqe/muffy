use crate::{
    cache::CacheError, document_parser::DocumentParseError, http_client::HttpClientError,
    sitemap::SitemapError,
};
use core::{
    error,
    fmt::{self, Display, Formatter},
    str::Utf8Error,
};
use http::StatusCode;
use muffy_validation::MarkupError;
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
    /// A document parse error.
    DocumentParse(DocumentParseError),
    /// An I/O error.
    Io(io::Error),
    /// An item error.
    Item(ItemError),
    /// An thread join error.
    Join(JoinError),
    /// A JSON serialization error.
    Json(serde_json::Error),
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
            Self::DocumentParse(error) => write!(formatter, "{error}"),
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Item(error) => write!(formatter, "{error}"),
            Self::Join(error) => write!(formatter, "{error}"),
            Self::Json(error) => write!(formatter, "{error}"),
            Self::Sled(error) => write!(formatter, "{error}"),
            Self::UrlParse(error) => write!(formatter, "{error}"),
            Self::Utf8(error) => write!(formatter, "{error}"),
            Self::Validation => write!(formatter, "validation failed"),
        }
    }
}

impl From<ItemError> for Error {
    fn from(error: ItemError) -> Self {
        Self::Item(error)
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

impl From<DocumentParseError> for Error {
    fn from(error: DocumentParseError) -> Self {
        Self::DocumentParse(error)
    }
}

impl From<JoinError> for Error {
    fn from(error: JoinError) -> Self {
        Self::Join(error)
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

impl From<Utf8Error> for Error {
    fn from(error: Utf8Error) -> Self {
        Self::Utf8(error)
    }
}

/// An element item error.
#[derive(Debug)]
pub enum ItemError {
    /// An invalid content type.
    ContentTypeInvalid {
        /// An actual content type.
        actual: String,
        /// An expected content type.
        expected: &'static str,
    },
    /// A document parse error.
    DocumentParse(DocumentParseError),
    /// An element not found.
    ElementNotFound(String),
    /// An HTTP client error.
    HttpClient(HttpClientError),
    /// An error status code in an HTTP response.
    HttpStatus(StatusCode),
    /// An invalid namespace.
    InvalidNamespace {
        /// An actual namespace.
        actual: Option<String>,
        /// An expected namespace.
        expected: &'static str,
    },
    /// An invalid scheme.
    InvalidScheme(String),
    /// A markup error.
    Markup(MarkupError),
    /// A sitemap parse error.
    Sitemap(SitemapError),
    /// A URL parse error.
    UrlParse(ParseError),
    /// A UTF-8 error.
    Utf8(Utf8Error),
    /// An XML syntax error.
    XmlSyntax(String),
}

impl error::Error for ItemError {}

impl Display for ItemError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::ContentTypeInvalid { actual, expected } => {
                write!(
                    formatter,
                    "content type expected {expected} but got {actual}"
                )
            }
            Self::DocumentParse(error) => write!(formatter, "{error}"),
            Self::ElementNotFound(name) => {
                write!(formatter, "element for #{name} not found")
            }
            Self::HttpClient(error) => write!(formatter, "{error}"),
            Self::HttpStatus(status) => write!(formatter, "invalid status {status}"),
            Self::InvalidNamespace { actual, expected } => {
                write!(
                    formatter,
                    "namespace expected {expected} but got {}",
                    actual.as_deref().unwrap_or("none")
                )
            }
            Self::InvalidScheme(scheme) => write!(formatter, "invalid scheme \"{scheme}\""),
            Self::Markup(error) => write!(formatter, "{error}"),
            Self::Sitemap(error) => write!(formatter, "{error}"),
            Self::UrlParse(error) => write!(formatter, "{error}"),
            Self::Utf8(error) => write!(formatter, "{error}"),
            Self::XmlSyntax(message) => write!(formatter, "invalid XML: {message}"),
        }
    }
}

impl Serialize for ItemError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<DocumentParseError> for ItemError {
    fn from(error: DocumentParseError) -> Self {
        Self::DocumentParse(error)
    }
}

impl From<HttpClientError> for ItemError {
    fn from(error: HttpClientError) -> Self {
        Self::HttpClient(error)
    }
}

impl From<url::ParseError> for ItemError {
    fn from(error: url::ParseError) -> Self {
        Self::UrlParse(error)
    }
}

impl From<Utf8Error> for ItemError {
    fn from(error: Utf8Error) -> Self {
        Self::Utf8(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_item_markup_error() {
        assert_eq!(
            format!(
                "{}",
                ItemError::Markup(MarkupError::UnknownTag("foo".into()))
            ),
            "unknown tag \"foo\""
        );
    }

    #[test]
    fn display_item_namespace_error() {
        assert_eq!(
            format!(
                "{}",
                ItemError::InvalidNamespace {
                    actual: None,
                    expected: "http://www.w3.org/2000/svg",
                }
            ),
            "namespace expected http://www.w3.org/2000/svg but got none"
        );
    }

    #[test]
    fn display_item_xml_syntax_error() {
        assert_eq!(
            format!(
                "{}",
                ItemError::XmlSyntax("Unexpected element in end phase".into())
            ),
            "invalid XML: Unexpected element in end phase"
        );
    }
}
