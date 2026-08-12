use crate::{error::MacroError, namespace::resolve_namespaces};
use alloc::collections::{BTreeMap, BTreeSet};
use muffy_rnc::{
    DefinitionSet, Grammar, GrammarContent, Identifier, Pattern as RncPattern, SchemaBody,
    defined_names, parse_schema,
};
use std::{fs::read_to_string, path::Path};

pub fn load_definitions(files: &[&str]) -> Result<BTreeMap<Identifier, RncPattern>, MacroError> {
    let mut definitions = DefinitionSet::default();

    for file in files {
        load_schema(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(file),
            &Default::default(),
            &mut definitions,
        )?;
    }

    Ok(definitions.into_patterns())
}

fn load_schema(
    path: &Path,
    overridden: &BTreeSet<Identifier>,
    definitions: &mut DefinitionSet,
) -> Result<(), MacroError> {
    // TODO Respect default namespaces of unprefixed names.
    let schema = resolve_namespaces(parse_schema(&read_to_string(path)?)?);

    match schema.body {
        SchemaBody::Grammar(grammar) | SchemaBody::Pattern(RncPattern::Grammar(grammar)) => {
            load_grammar(
                &grammar,
                path.parent().ok_or(MacroError::NoParentDirectory)?,
                overridden,
                definitions,
            )?;
        }
        SchemaBody::Pattern(_) => return Err(MacroError::RncSyntax("top-level pattern")),
    }

    Ok(())
}

pub fn load_grammar(
    grammar: &Grammar,
    directory: &Path,
    overridden: &BTreeSet<Identifier>,
    definitions: &mut DefinitionSet,
) -> Result<(), MacroError> {
    for content in &grammar.contents {
        match content {
            GrammarContent::Definition(definition) => {
                if !overridden.contains(&definition.name) {
                    definitions.define(definition)?;
                }
            }
            GrammarContent::Div(grammar) => {
                load_grammar(grammar, directory, overridden, definitions)?
            }
            GrammarContent::Include(include) => {
                load_schema(
                    &directory.join(&include.uri),
                    &overridden
                        .iter()
                        .cloned()
                        .chain(include.grammar.iter().flat_map(defined_names))
                        .collect(),
                    definitions,
                )?;

                if let Some(grammar) = &include.grammar {
                    load_grammar(grammar, directory, overridden, definitions)?;
                }
            }
            GrammarContent::Annotation(_) | GrammarContent::Start { .. } => {}
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::MacroError;
    use muffy_rnc::{NameClass, SchemaError};
    use pretty_assertions::assert_eq;
    use std::fs::write;
    use tempfile::tempdir;

    fn load(files: &[(&str, &str)]) -> Result<Vec<RncPattern>, MacroError> {
        let directory = tempdir().unwrap();

        for (name, source) in files {
            write(directory.path().join(name), source).unwrap();
        }

        let mut definitions = DefinitionSet::default();

        load_schema(
            &directory.path().join(files[0].0),
            &Default::default(),
            &mut definitions,
        )?;

        Ok(definitions.into_patterns().into_values().collect())
    }

    fn attribute(name: &str) -> RncPattern {
        let SchemaBody::Grammar(grammar) =
            parse_schema(&format!("root = attribute {name} {{ text }}"))
                .unwrap()
                .body
        else {
            panic!("grammar expected");
        };
        let GrammarContent::Definition(definition) = &grammar.contents[0] else {
            panic!("definition expected");
        };

        definition.pattern.clone()
    }

    #[test]
    fn load_included_definitions() {
        assert_eq!(
            load(&[("main.rnc", "include \"a.rnc\""), ("a.rnc", "root = empty")]).unwrap(),
            vec![RncPattern::Empty]
        );
    }

    #[test]
    fn resolve_empty_namespace_wildcard_attribute() {
        assert_eq!(
            load(&[(
                "main.rnc",
                "namespace none = \"\"\nroot = attribute none:* { text }"
            )])
            .unwrap(),
            vec![RncPattern::Attribute {
                name_class: NameClass::Namespace(None),
                pattern: RncPattern::Text.into(),
            }]
        );
    }

    #[test]
    fn replace_plain_definition_in_override() {
        assert_eq!(
            load(&[
                ("main.rnc", "include \"a.rnc\" { root = notAllowed }"),
                ("a.rnc", "root = empty"),
            ])
            .unwrap(),
            vec![RncPattern::NotAllowed]
        );
    }

    #[test]
    fn replace_plain_definition_in_override_div() {
        assert_eq!(
            load(&[
                (
                    "main.rnc",
                    "include \"a.rnc\" { div { root = notAllowed } }"
                ),
                ("a.rnc", "root = empty"),
            ])
            .unwrap(),
            vec![RncPattern::NotAllowed]
        );
    }

    #[test]
    fn replace_combined_definition_in_override() {
        assert_eq!(
            load(&[
                (
                    "main.rnc",
                    "include \"a.rnc\" { root &= attribute foo { text } }"
                ),
                ("a.rnc", "root = attribute bar { text }"),
            ])
            .unwrap(),
            vec![attribute("foo")]
        );
    }

    #[test]
    fn keep_definitions_outside_included_schema_on_override() {
        assert_eq!(
            load(&[
                (
                    "main.rnc",
                    "root &= attribute foo { text }\ninclude \"a.rnc\" { root = attribute baz { text } }"
                ),
                ("a.rnc", "root = attribute bar { text }"),
            ])
            .unwrap(),
            vec![RncPattern::Interleave(vec![
                attribute("foo"),
                attribute("baz")
            ])]
        );
    }

    #[test]
    fn merge_override_definition_with_definition_outside_included_schema() {
        assert_eq!(
            load(&[
                (
                    "main.rnc",
                    "root |= attribute foo { text }\ninclude \"a.rnc\" { root |= attribute baz { text } }"
                ),
                ("a.rnc", "root &= attribute bar { text }"),
            ])
            .unwrap(),
            vec![RncPattern::Choice(vec![
                attribute("foo"),
                attribute("baz")
            ])]
        );
    }

    #[test]
    fn replace_definition_in_nested_include() {
        assert_eq!(
            load(&[
                (
                    "main.rnc",
                    "include \"a.rnc\" { root &= attribute foo { text } }"
                ),
                ("a.rnc", "include \"b.rnc\""),
                ("b.rnc", "root &= attribute bar { text }"),
            ])
            .unwrap(),
            vec![attribute("foo")]
        );
    }

    #[test]
    fn fail_on_conflicting_combine_operators_across_include() {
        assert!(matches!(
            load(&[
                (
                    "main.rnc",
                    "root |= attribute foo { text }\ninclude \"a.rnc\""
                ),
                ("a.rnc", "root &= attribute bar { text }"),
            ]),
            Err(MacroError::RncSchema(SchemaError::CombineConflict(_)))
        ));
    }
}
