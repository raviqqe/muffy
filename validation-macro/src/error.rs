use core::{
    error::Error,
    fmt::{self, Display, Formatter},
};
use muffy_rnc::ParseError;
use std::io;

/// A macro error.
#[derive(Debug)]
pub enum MacroError {
    Io(io::Error),
    NoParentDirectory,
    RncParse(ParseError),
    RncSyntax(&'static str),
}

impl Display for MacroError {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::NoParentDirectory => write!(formatter, "no parent directory"),
            Self::RncSyntax(error) => write!(formatter, "unexpected RNC syntax: {error}"),
            Self::RncParse(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for MacroError {}

impl From<io::Error> for MacroError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<ParseError> for MacroError {
    fn from(error: ParseError) -> Self {
        Self::RncParse(error)
    }
}
