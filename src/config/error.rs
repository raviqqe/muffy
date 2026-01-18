use core::error::Error;
use core::fmt;
use core::fmt::Display;
use core::fmt::Formatter;

/// A configuration error.
#[derive(Debug)]
pub enum ConfigError {
    /// Invalid site exclusion.
    InvalidSiteExclude(String),
    /// An invalid status code.
    HttpInvalidStatus(http::status::InvalidStatusCode),
    /// An invalid header name.
    HttpInvalidHeaderName(http::header::InvalidHeaderName),
    /// An invalid header value.
    HttpInvalidHeaderValue(http::header::InvalidHeaderValue),
}

impl Display for ConfigError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::InvalidSiteExclude(url) => {
                write!(formatter, "exclude field must be true if present: {url}")
            }
            ConfigError::HttpInvalidStatus(error) => {
                write!(formatter, "{error}")
            }
            ConfigError::HttpInvalidHeaderName(error) => {
                write!(formatter, "{error}")
            }
            ConfigError::HttpInvalidHeaderValue(error) => {
                write!(formatter, "{error}")
            }
        }
    }
}

impl Error for ConfigError {}

impl From<http::status::InvalidStatusCode> for ConfigError {
    fn from(error: http::status::InvalidStatusCode) -> Self {
        Self::HttpInvalidStatus(error)
    }
}

impl From<http::header::InvalidHeaderName> for ConfigError {
    fn from(error: http::header::InvalidHeaderName) -> Self {
        Self::HttpInvalidHeaderName(error)
    }
}

impl From<http::header::InvalidHeaderValue> for ConfigError {
    fn from(error: http::header::InvalidHeaderValue) -> Self {
        Self::HttpInvalidHeaderValue(error)
    }
}
