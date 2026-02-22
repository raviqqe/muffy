//! The parser for Relax NG Compact Syntax.

mod ast;
mod parse;

pub use self::{ast::*, parse::*};

#[cfg(test)]
mod tests {
    use super::*;
    use glob::glob;
    use std::fs::read_to_string;

    #[test]
    fn parse_html_svg_rnc_files() {
        let validator_directory = format!("{}/../vendor/validator", env!("CARGO_MANIFEST_DIR"));
        let mut files = glob(&format!("{}/**/*.rnc", validator_directory))
            .unwrap()
            .into_iter()
            .chain(glob(&format!("{}/**/*.rnc", validator_directory)).unwrap())
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        files.sort();

        let mut failures = Vec::new();
        for file in files {
            let contents = read_to_string(&file)
                .unwrap_or_else(|error| panic!("failed to read {}: {error}", file.display()));
            if let Err(error) = parse_schema(&contents) {
                failures.push(format!("{}: {error}", file.display()));
            }
        }

        assert!(
            failures.is_empty(),
            "failed to parse .rnc files:\n{}",
            failures.join("\n")
        );
    }
}
