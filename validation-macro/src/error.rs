use core::{
    error::Error,
    fmt::{self, Display, Formatter},
};
use std::io;

/// A macro error.
#[derive(Debug)]
pub enum MacroError {
    Io(io::Error),
    NoParentDirectory,
}

impl Display for MacroError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "IO error: {error}"),
            Self::NoParentDirectory => write!(formatter, "no parent directory"),
        }
    }
}

impl Error for MacroError {}

impl From<io::Error> for MacroError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
