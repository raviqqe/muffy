use core::{
    convert::Infallible,
    error::Error,
    fmt::{self, Display, Formatter},
    str::Utf8Error,
};
use std::sync::PoisonError;

/// A CSS parse error.
#[derive(Debug, Eq, PartialEq)]
pub enum CssError {
    /// A poisoned lock.
    Poison,
    /// A syntax error.
    Syntax(String),
    /// A UTF-8 error.
    Utf8(Utf8Error),
}

impl Error for CssError {}

impl Display for CssError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Poison => write!(formatter, "poisoned lock"),
            Self::Syntax(message) => write!(formatter, "{message}"),
            Self::Utf8(error) => write!(formatter, "{error}"),
        }
    }
}

impl From<Infallible> for CssError {
    fn from(error: Infallible) -> Self {
        match error {}
    }
}

impl<T> From<PoisonError<T>> for CssError {
    fn from(_: PoisonError<T>) -> Self {
        Self::Poison
    }
}

impl From<Utf8Error> for CssError {
    fn from(error: Utf8Error) -> Self {
        Self::Utf8(error)
    }
}
