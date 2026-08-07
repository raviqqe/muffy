use crate::ast::Identifier;
use core::{
    error::Error,
    fmt::{self, Display, Formatter},
};

/// A schema error.
#[derive(Debug, PartialEq, Eq)]
pub enum SchemaError {
    /// Conflicting combine operators.
    CombineConflict(Identifier),
}

impl Display for SchemaError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::CombineConflict(name) => {
                write!(formatter, "conflicting combine operators: {name}")
            }
        }
    }
}

impl Error for SchemaError {}
