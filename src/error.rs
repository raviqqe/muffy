use core::error;
use core::fmt::{self, Display, Formatter};
use core::str::Utf8Error;
use reqwest::StatusCode;
use std::io;
use alloc::sync::Arc;
use tokio::sync::AcquireError;
use tokio::task::JoinError;
use url::ParseError;

#[derive(Debug)]
pub enum Error {
    Acquire(AcquireError),
    HtmlParse(io::Error),
    InvalidStatus(StatusCode),
    Io(io::Error),
    Join(JoinError),
    Reqwest(Arc<reqwest::Error>),
    UrlParse(ParseError),
    Utf8(Utf8Error),
}

impl error::Error for Error {}

impl Display for Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Acquire(error) => write!(formatter, "{error}"),
            Self::HtmlParse(error) => write!(formatter, "{error}"),
            Self::InvalidStatus(status) => write!(formatter, "invalid status {status}"),
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Join(error) => write!(formatter, "{error}"),
            Self::Reqwest(error) => write!(formatter, "{error}"),
            Self::UrlParse(error) => write!(formatter, "{error}"),
            Self::Utf8(error) => write!(formatter, "{error}"),
        }
    }
}

impl From<AcquireError> for Error {
    fn from(error: AcquireError) -> Self {
        Self::Acquire(error)
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<JoinError> for Error {
    fn from(error: JoinError) -> Self {
        Self::Join(error)
    }
}

impl From<url::ParseError> for Error {
    fn from(error: url::ParseError) -> Self {
        Self::UrlParse(error)
    }
}

impl From<Arc<reqwest::Error>> for Error {
    fn from(error: Arc<reqwest::Error>) -> Self {
        Self::Reqwest(error)
    }
}

impl From<Utf8Error> for Error {
    fn from(error: Utf8Error) -> Self {
        Self::Utf8(error)
    }
}
