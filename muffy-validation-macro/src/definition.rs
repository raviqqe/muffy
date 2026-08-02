use crate::error::MacroError;
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
                definitions,
                path.parent().ok_or(MacroError::NoParentDirectory)?,
                false,
            )?;
        }
        SchemaBody::Pattern(_) => return Err(MacroError::RncSyntax("top-level pattern")),
    }

    Ok(())
}

pub fn load_grammar(
    grammar: &Grammar,
    definitions: &mut DefinitionSet,
    directory: &Path,
    replace: bool,
) -> Result<(), MacroError> {
    for content in &grammar.contents {
        match content {
            GrammarContent::Definition(definition) => {
                load_definition(definition, definitions, replace);
            }
            GrammarContent::Div(grammar) => load_grammar(grammar, definitions, directory, replace)?,
            GrammarContent::Include(include) => {
                load_schema(&directory.join(&include.uri), definitions)?;

                if let Some(grammar) = &include.grammar {
                    load_grammar(grammar, definitions, directory, true)?;
                }
            }
            GrammarContent::Annotation(_) | GrammarContent::Start { .. } => {}
        }
    }

    Ok(())
}

// A name can be defined once without a combine operator and multiple times
// with a consistent one, in any order across schema files. Definitions in
// include blocks replace included ones instead.
fn load_definition(definition: &Definition, definitions: &mut DefinitionSet, replace: bool) {
    let pattern = definition.pattern.clone();

    if let Some(combine) = definition.combine {
        if let Some((operator, existing)) = definitions.get_mut(&definition.name) {
            combine_patterns(existing, pattern, combine);
            operator.get_or_insert(combine);
        } else {
            definitions.insert(definition.name.clone(), (Some(combine), pattern));
        }
    } else if !replace
        && let Some((Some(operator), existing)) = definitions.get_mut(&definition.name)
    {
        combine_patterns(existing, pattern, *operator);
    } else {
        definitions.insert(definition.name.clone(), (None, pattern));
    }
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
