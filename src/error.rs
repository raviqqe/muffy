use alloc::sync::Arc;
use core::error;
use core::fmt::{self, Display, Formatter};
use core::str::Utf8Error;
use reqwest::StatusCode;
use std::io;
use tokio::sync::AcquireError;
use tokio::task::JoinError;
use url::ParseError;

#[derive(Debug)]
pub enum Error {
    Acquire(AcquireError),
    Get {
        url: String,
        source: Arc<reqwest::Error>,
    },
    HtmlParse {
        url: String,
        source: io::Error,
    },
    InvalidStatus {
        url: String,
        status: StatusCode,
    },
    Io(io::Error),
    Join(JoinError),
    UrlParse {
        url: String,
        source: ParseError,
    },
    Utf8(Utf8Error),
}

impl error::Error for Error {}

impl Display for Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Acquire(error) => write!(formatter, "{error}"),
            Self::Get { url, source } => {
                write!(formatter, "failed to GET {url}: {source}")
            }
            Self::HtmlParse { url, source } => {
                write!(formatter, "failed to parse HTML from {url}: {source}")
            }
            Self::InvalidStatus { url, status } => {
                write!(formatter, "invalid status {status} at {url}")
            }
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Join(error) => write!(formatter, "{error}"),
            Self::UrlParse { url, source } => {
                write!(formatter, "failed to parse URL {url}: {source}")
            }
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

impl From<Utf8Error> for Error {
    fn from(error: Utf8Error) -> Self {
        Self::Utf8(error)
    }
}
