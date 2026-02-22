use core::{
    error::Error,
    fmt,
    fmt::{Display, Formatter},
};
use std::{io, path::PathBuf};
use url::ParseError;

/// A configuration error.
#[derive(Debug)]
pub enum ConfigError {
    /// Circular configuration extensions.
    CircularConfigFiles(Vec<PathBuf>),
    /// Circular site configurations.
    CircularSiteConfigs(Vec<String>),
    /// An invalid status code.
    HttpInvalidStatus(http::status::InvalidStatusCode),
    /// An invalid header name.
    HttpInvalidHeaderName(http::header::InvalidHeaderName),
    /// An invalid header value.
    HttpInvalidHeaderValue(http::header::InvalidHeaderValue),
    /// An I/O error while reading configuration.
    Io(io::Error),
    /// Missing parent configuration.
    MissingParentConfig(String),
    /// Multiple default site configurations.
    MultipleDefaultSiteConfigs(Vec<String>),
    /// A regular expression error.
    Regex(regex::Error),
    /// A TOML deserialization error.
    TomlDeserialize(::toml::de::Error),
    /// A URL parse error.
    UrlParse(ParseError),
}

impl Display for ConfigError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::CircularConfigFiles(paths) => {
                let paths = paths
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(" -> ");
                write!(formatter, "circular configuration files: {paths}")
            }
            Self::CircularSiteConfigs(names) => {
                write!(
                    formatter,
                    "circular site configurations: {}",
                    names.join(" -> ")
                )
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
            Self::Io(error) => {
                write!(formatter, "{error}")
            }
            Self::MissingParentConfig(name) => {
                write!(formatter, "missing parent configuration: {name}")
            }
            Self::MultipleDefaultSiteConfigs(names) => {
                write!(
                    formatter,
                    "multiple default site configurations: {}",
                    names.join(", ")
                )
            }
            Self::Regex(error) => {
                write!(formatter, "{error}")
            }
            Self::TomlDeserialize(error) => {
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

impl From<io::Error> for ConfigError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<::toml::de::Error> for ConfigError {
    fn from(error: ::toml::de::Error) -> Self {
        Self::TomlDeserialize(error)
    }
}

impl From<ParseError> for ConfigError {
    fn from(error: ParseError) -> Self {
        Self::UrlParse(error)
    }
}
