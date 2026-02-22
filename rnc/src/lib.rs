//! The parser for Relax NG Compact Syntax.

mod ast;
mod parse;

pub use self::{ast::*, parse::*};

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::{fs::read_to_string, path::PathBuf};

    #[rstest]
    fn parse_file(#[files("../vendor/validator/**/schema/**/*.rnc")] path: PathBuf) {
        parse_schema(&read_to_string(&path).unwrap()).unwrap();
    }
}
