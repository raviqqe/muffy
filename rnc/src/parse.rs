//! A parser for Relax NG Compact Syntax.

mod error;
mod parser;

pub use self::error::ParseError;
use self::parser::{blank0, schema};
use crate::ast::Schema;
use nom::{Parser, combinator::all_consuming, sequence::delimited};

/// Parses a schema.
pub fn parse_schema(input: &str) -> Result<Schema, ParseError> {
    Ok(all_consuming(delimited(blank0, schema, blank0))
        .parse(input)?
        .1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_debug_snapshot;
    use rstest::rstest;
    use std::{
        fs::read_to_string,
        path::{Path, PathBuf},
    };

    #[rstest]
    fn parse_file(#[files("../vendor/validator/**/schema/**/*.rnc")] path: PathBuf) {
        assert_debug_snapshot!(
            path.strip_prefix(Path::new("../vendor/validator").canonicalize().unwrap())
                .unwrap()
                .display()
                .to_string(),
            parse_schema(&read_to_string(&path).unwrap()).unwrap()
        );
    }
}
