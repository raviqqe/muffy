use core::error;
use core::fmt::{self, Display, Formatter};
use std::io;
use tokio::task::JoinError;

#[derive(Debug)]
pub enum Error {
    Get { url: String, source: reqwest::Error },
    HtmlParse { url: String, source: io::Error },
    Io(io::Error),
    Join(JoinError),
}

impl error::Error for Error {}

impl Display for Error {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Get { url, source } => {
                write!(formatter, "failed to GET {url}: {source}")
            }
            Self::HtmlParse { url, source } => {
                write!(formatter, "failed to parse HTML from {url}: {source}")
            }
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Join(error) => write!(formatter, "{error}"),
        }
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
