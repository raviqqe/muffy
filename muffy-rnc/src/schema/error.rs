use crate::ast::Identifier;
use core::{
    error::Error,
    fmt::{self, Display, Formatter},
};

/// A definition error.
#[derive(Debug, PartialEq, Eq)]
pub enum DefinitionError {
    /// Conflicting combine operators.
    CombineConflict(Identifier),
}

impl Display for DefinitionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::CombineConflict(name) => {
                write!(formatter, "conflicting combine operators: {name}")
            }
        }
    }
}

impl Error for DefinitionError {}
