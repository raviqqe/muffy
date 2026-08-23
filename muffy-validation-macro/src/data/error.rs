use core::{
    error::Error,
    fmt::{self, Display, Formatter},
};

/// An XSD pattern error.
#[derive(Debug, PartialEq)]
pub enum XsdPatternError {
    InvalidRange,
    Regex(regex::Error),
    TrailingBackslash,
    UnbalancedParentheses,
    UnclosedClass,
    UnescapedBracket,
    UnescapedDash,
    UnknownEscape(char),
}

impl Display for XsdPatternError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRange => write!(formatter, "multi-character escape in range"),
            Self::Regex(error) => write!(formatter, "{error}"),
            Self::TrailingBackslash => write!(formatter, "trailing backslash"),
            Self::UnbalancedParentheses => write!(formatter, "unbalanced parentheses"),
            Self::UnclosedClass => write!(formatter, "unclosed character class"),
            Self::UnescapedBracket => write!(formatter, "unescaped bracket in character class"),
            Self::UnescapedDash => write!(formatter, "unescaped dash in character class"),
            Self::UnknownEscape(character) => write!(formatter, "unknown escape: \\{character}"),
        }
    }
}

impl Error for XsdPatternError {}

impl From<regex::Error> for XsdPatternError {
    fn from(error: regex::Error) -> Self {
        Self::Regex(error)
    }
}
