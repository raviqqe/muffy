use core::{
    error::Error,
    fmt::{self, Display, Formatter},
};
use

/// A macro error.
#[derive(Debug, PartialEq, Eq)]
pub enum MacroError {
    Io(io::Error),
    NoParentDirectory
}

impl Display for MacroError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
    }
}

impl Error for MacroError {}

impl<'a> From<nom::Err<nom::error::Error<&'a str>>> for MacroError {
    fn from(error: nom::Err<nom::error::Error<&'a str>>) -> Self {
        Self {
            message: match error {
                nom::Err::Incomplete(_) => "incomplete input".into(),
                nom::Err::Error(error) | nom::Err::Failure(error) => error.to_string(),
            },
        }
    }
}
