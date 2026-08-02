use crate::{error::MacroError, name::identifier_string};
use alloc::collections::BTreeMap;
use core::mem::replace;
use muffy_rnc::{
    Combine, Definition, Grammar, GrammarContent, Identifier, Pattern as RncPattern, SchemaBody,
    parse_schema,
};
use std::{fs::read_to_string, path::Path};

pub type DefinitionSet = BTreeMap<Identifier, (Option<Combine>, RncPattern)>;

pub fn load_definitions(files: &[&str]) -> Result<BTreeMap<Identifier, RncPattern>, MacroError> {
    let mut definitions = DefinitionSet::default();

    for file in files {
        load_schema(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(file),
            &mut definitions,
        )?;
    }

    Ok(definitions
        .into_iter()
        .map(|(name, (_, pattern))| (name, pattern))
        .collect())
}

fn load_schema(path: &Path, definitions: &mut DefinitionSet) -> Result<(), MacroError> {
    let schema = parse_schema(&read_to_string(path)?)?;

    // We do not use the declarations.
    // TODO Respect namespace declarations.

    match schema.body {
        SchemaBody::Grammar(grammar) | SchemaBody::Pattern(RncPattern::Grammar(grammar)) => {
            load_grammar(
                &grammar,
                path.parent().ok_or(MacroError::NoParentDirectory)?,
                false,
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
    replace: bool,
    definitions: &mut DefinitionSet,
) -> Result<(), MacroError> {
    for content in &grammar.contents {
        match content {
            GrammarContent::Definition(definition) => {
                load_definition(definition, replace, definitions)?
            }
            GrammarContent::Div(grammar) => load_grammar(grammar, directory, replace, definitions)?,
            GrammarContent::Include(include) => {
                load_schema(&directory.join(&include.uri), definitions)?;

                if let Some(grammar) = &include.grammar {
                    load_grammar(grammar, directory, true, definitions)?;
                }
            }
            GrammarContent::Annotation(_) | GrammarContent::Start { .. } => {}
        }
    }

    Ok(())
}

fn load_definition(
    definition: &Definition,
    replace: bool,
    definitions: &mut DefinitionSet,
) -> Result<(), MacroError> {
    let pattern = definition.pattern.clone();

    if let Some(combine) = definition.combine
        && let Some((operator, existing)) = definitions.get_mut(&definition.name)
    {
        if let Some(operator) = *operator
            && operator != combine
        {
            return Err(MacroError::CombineConflict(identifier_string(
                &definition.name,
            )));
        }

        combine_patterns(existing, pattern, combine);
        *operator = Some(combine);
    } else if !replace
        && definition.combine.is_none()
        && let Some((Some(operator), existing)) = definitions.get_mut(&definition.name)
    {
        combine_patterns(existing, pattern, *operator);
    } else {
        definitions.insert(definition.name.clone(), (definition.combine, pattern));
    }

    Ok(())
}

fn combine_patterns(existing: &mut RncPattern, new: RncPattern, combine: Combine) {
    match combine {
        Combine::Choice => match existing {
            RncPattern::Choice(choices) => choices.push(new),
            RncPattern::NotAllowed => *existing = new,
            RncPattern::Attribute { .. }
            | RncPattern::Data { .. }
            | RncPattern::Element { .. }
            | RncPattern::Empty
            | RncPattern::External(_)
            | RncPattern::Grammar(_)
            | RncPattern::Group(_)
            | RncPattern::Interleave(_)
            | RncPattern::List(_)
            | RncPattern::Many0(_)
            | RncPattern::Many1(_)
            | RncPattern::Name(_)
            | RncPattern::Optional(_)
            | RncPattern::Text
            | RncPattern::Value { .. } => {
                let old = replace(existing, RncPattern::Choice(vec![]));

                if let RncPattern::Choice(choices) = existing {
                    choices.push(old);
                    choices.push(new);
                }
            }
        },
        Combine::Interleave => match existing {
            RncPattern::Interleave(patterns) => patterns.push(new),
            RncPattern::NotAllowed => *existing = new,
            RncPattern::Attribute { .. }
            | RncPattern::Choice(_)
            | RncPattern::Data { .. }
            | RncPattern::Element { .. }
            | RncPattern::Empty
            | RncPattern::External(_)
            | RncPattern::Grammar(_)
            | RncPattern::Group(_)
            | RncPattern::List(_)
            | RncPattern::Many0(_)
            | RncPattern::Many1(_)
            | RncPattern::Name(_)
            | RncPattern::Optional(_)
            | RncPattern::Text
            | RncPattern::Value { .. } => {
                let old = replace(existing, RncPattern::Interleave(vec![]));

                if let RncPattern::Interleave(patterns) = existing {
                    patterns.push(old);
                    patterns.push(new);
                }
            }
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load(
        source: &str,
        replace: bool,
        definitions: &mut DefinitionSet,
    ) -> Result<(), MacroError> {
        let SchemaBody::Grammar(grammar) = parse_schema(source).unwrap().body else {
            panic!("grammar expected");
        };

        load_grammar(&grammar, Path::new("."), replace, definitions)
    }

    #[test]
    fn merge_combined_definitions() {
        assert!(
            load(
                "root &= attribute foo { text }\nroot &= attribute bar { text }",
                false,
                &mut DefinitionSet::default(),
            )
            .is_ok()
        );
    }

    #[test]
    fn replace_plain_definition_in_override() {
        let mut definitions = DefinitionSet::default();

        load("root = empty", false, &mut definitions).unwrap();
        load("root = notAllowed", true, &mut definitions).unwrap();

        assert_eq!(
            definitions
                .values()
                .map(|(_, pattern)| pattern)
                .collect::<Vec<_>>(),
            vec![&RncPattern::NotAllowed]
        );
    }

    #[test]
    fn fail_on_conflicting_combine_operators() {
        assert!(matches!(
            load(
                "root |= attribute foo { text }\nroot &= attribute bar { text }",
                false,
                &mut DefinitionSet::default(),
            ),
            Err(MacroError::CombineConflict(_))
        ));
    }

    #[test]
    fn fail_on_combine_operator_conflicting_with_merged_plain_definition() {
        assert!(matches!(
            load(
                "root |= attribute foo { text }\nroot = empty\nroot &= attribute bar { text }",
                false,
                &mut DefinitionSet::default(),
            ),
            Err(MacroError::CombineConflict(_))
        ));
    }
}
