use core::{
    error::Error,
    fmt::{self, Display, Formatter},
};

/// A parse error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    message: String,
}

impl ParseError {
    pub fn from_nom<'input>(error: nom::Err<nom::error::Error<&'input str>>) -> Self {
        Self {
            message: match error {
                nom::Err::Incomplete(_) => "incomplete input".to_string(),
                nom::Err::Error(error) | nom::Err::Failure(error) => error.to_string(),
            },
        }
    }
}

impl Display for ParseError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.message)
    }
}

impl Error for ParseError {}
