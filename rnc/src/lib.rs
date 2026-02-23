//! The parser for Relax NG Compact Syntax.

mod ast;
mod parse;

pub use self::{ast::*, parse::*};

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_debug_snapshot;
    use rstest::rstest;
    use std::{fs::read_to_string, path::PathBuf};

    #[rstest]
    fn parse_file(#[files("../vendor/validator/**/schema/**/*.rnc")] path: PathBuf) {
        assert_debug_snapshot!(
            path.display().to_string(),
            parse_schema(&read_to_string(&path).unwrap()).unwrap()
        );
    }
}
