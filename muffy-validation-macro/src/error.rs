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
    RncPattern(&'static str),
    RncSyntax(&'static str),
}

impl Display for MacroError {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::NoParentDirectory => write!(formatter, "no parent directory"),
            Self::RncParse(error) => write!(formatter, "{error}"),
            Self::RncPattern(name) => write!(formatter, "unexpected RNC pattern: {name}"),
            Self::RncSyntax(name) => write!(formatter, "unexpected RNC syntax: {name}"),
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
