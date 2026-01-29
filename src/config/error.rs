use core::{
    error::Error,
    fmt,
    fmt::{Display, Formatter},
};
use url::ParseError;

/// A configuration error.
#[derive(Debug)]
pub enum ConfigError {
    /// Default site missing.
    DefaultSiteMissing,
    /// Invalid site ignorance.
    InvalidSiteIgnore(String),
    /// An invalid status code.
    HttpInvalidStatus(http::status::InvalidStatusCode),
    /// An invalid header name.
    HttpInvalidHeaderName(http::header::InvalidHeaderName),
    /// An invalid header value.
    HttpInvalidHeaderValue(http::header::InvalidHeaderValue),
    /// A regular expression error.
    Regex(regex::Error),
    /// A URL parse error.
    UrlParse(ParseError),
}

impl Display for ConfigError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::DefaultSiteMissing => {
                write!(formatter, "default site missing in configuration file")
            }
            Self::InvalidSiteIgnore(url) => {
                write!(formatter, "ignore field must be true if present: {url}")
            }
            Self::HttpInvalidStatus(error) => {
                write!(formatter, "{error}")
            }
            Self::HttpInvalidHeaderName(error) => {
                write!(formatter, "{error}")
            }
            Self::HttpInvalidHeaderValue(error) => {
                write!(formatter, "{error}")
            }
            Self::Regex(error) => {
                write!(formatter, "{error}")
            }
            Self::UrlParse(error) => {
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

impl From<regex::Error> for ConfigError {
    fn from(error: regex::Error) -> Self {
        Self::Regex(error)
    }
}

impl From<ParseError> for ConfigError {
    fn from(error: ParseError) -> Self {
        Self::UrlParse(error)
    }
}
