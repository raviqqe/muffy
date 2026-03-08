use core::{
    error::Error,
    fmt::{self, Display, Formatter},
};

/// A parse error.
#[derive(Debug, PartialEq, Eq)]
pub struct ParseError {
    message: String,
}

impl Display for ParseError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl Error for ParseError {}

impl<'a> From<nom::Err<nom::error::Error<&'a str>>> for ParseError {
    fn from(error: nom::Err<nom::error::Error<&'a str>>) -> Self {
        Self {
            message: match error {
                nom::Err::Incomplete(_) => "incomplete input".into(),
                nom::Err::Error(error) | nom::Err::Failure(error) => error.to_string(),
            },
        }
    }
}
