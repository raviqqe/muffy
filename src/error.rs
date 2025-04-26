use core::error;
use core::fmt::{self, Display, Formatter};
use std::io;
use tokio::sync::AcquireError;
use tokio::task::JoinError;
use url::ParseError;

#[derive(Debug)]
pub enum Error {
    Acquire(AcquireError),
    Get { url: String, source: reqwest::Error },
    HtmlParse { url: String, source: io::Error },
    Io(io::Error),
    Join(JoinError),
    UrlParse { url: String, source: ParseError },
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
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Join(error) => write!(formatter, "{error}"),
            Self::UrlParse { url, source } => {
                write!(formatter, "failed to parse URL {url}: {source}")
            }
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
