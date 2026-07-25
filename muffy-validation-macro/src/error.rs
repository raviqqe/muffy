use core::{
    error::Error,
    fmt::{self, Display, Formatter},
};
use muffy_rnc::ParseError;
use std::io;

/// A macro error.
#[derive(Debug)]
pub enum MacroError {
    CircularReference(String),
    Io(io::Error),
    NoParentDirectory,
    PatternLimit(&'static str),
    RncParse(ParseError),
    RncPattern(&'static str),
    RncSyntax(&'static str),
    UndefinedReference(String),
}

impl Display for MacroError {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        match self {
            Self::CircularReference(name) => write!(formatter, "circular reference: {name}"),
            Self::Io(error) => write!(formatter, "{error}"),
            Self::NoParentDirectory => write!(formatter, "no parent directory"),
            Self::PatternLimit(name) => write!(formatter, "too many {name}"),
            Self::RncParse(error) => write!(formatter, "{error}"),
            Self::RncPattern(name) => write!(formatter, "unexpected RNC pattern: {name}"),
            Self::RncSyntax(name) => write!(formatter, "unexpected RNC syntax: {name}"),
            Self::UndefinedReference(name) => write!(formatter, "undefined reference: {name}"),
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
