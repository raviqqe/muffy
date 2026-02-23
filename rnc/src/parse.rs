//! A parser for Relax NG Compact Syntax.

mod error;
mod parser;

pub use self::error::ParseError;
use crate::ast::Schema;
use nom::{Parser, combinator::all_consuming, sequence::delimited};

/// Parses a schema.
pub fn parse_schema(input: &str) -> Result<Schema, ParseError> {
    Ok(all_consuming(delimited(
        parser::whitespace0,
        parser::schema,
        parser::whitespace0,
    ))
    .parse(input)?
    .1)
}
