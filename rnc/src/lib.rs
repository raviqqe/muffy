//! The parser for Relax NG Compact Syntax.

pub mod ast;
pub mod parse;

pub use self::{
    ast::{
        Annotation, AnnotationAttribute, Combine, DatatypesDeclaration, Declaration, Definition,
        Grammar, GrammarItem, Inherit, Name, NameClass, NamespaceDeclaration, Parameter, Pattern,
        Schema, SchemaBody,
    },
    parse::{ParseError, parse_schema},
};

#[cfg(test)]
mod tests {
    use super::parse_schema;
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    #[test]
    fn parse_html_svg_rnc_files() {
        let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("workspace root should exist");
        let validator_root = workspace_root.join("vendor").join("validator");
        let mut files = Vec::new();

        collect_rnc_files(&validator_root, &mut files);
        files.sort();

        assert!(
            !files.is_empty(),
            "expected to find HTML/SVG .rnc files in {}",
            validator_root.display()
        );

        let mut failures = Vec::new();
        for file in files {
            let contents = fs::read_to_string(&file)
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

    fn collect_rnc_files(directory: &Path, files: &mut Vec<PathBuf>) {
        let entries = fs::read_dir(directory)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", directory.display()));

        for entry_result in entries {
            let entry = entry_result.expect("failed to read directory entry");
            let path = entry.path();
            if path.is_dir() {
                collect_rnc_files(&path, files);
            } else if is_html_svg_rnc(&path) {
                files.push(path);
            }
        }
    }

    fn is_html_svg_rnc(path: &Path) -> bool {
        let is_rnc = path
            .extension()
            .and_then(|extension| extension.to_str())
            .map_or(false, |extension| extension == "rnc");
        if !is_rnc {
            return false;
        }

        let path_string = path.to_string_lossy();
        let is_schema = path_string.contains("/schema/");
        let is_html_or_svg = path_string.contains("/schema/html5/")
            || path_string.contains("/schema/svg11/")
            || path_string.contains("/schema/its2/its20-html5")
            || path_string.contains("/schema/.drivers/html5")
            || path_string.contains("/schema/.drivers/svg");

        is_schema && is_html_or_svg
    }
}
