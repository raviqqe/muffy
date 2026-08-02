use crate::error::MacroError;
use alloc::collections::BTreeMap;
use core::mem::replace;
use muffy_rnc::{
    Combine, Grammar, GrammarContent, Identifier, Pattern as RncPattern, SchemaBody, parse_schema,
};
use std::{fs::read_to_string, path::Path};

pub fn load_definitions() -> Result<BTreeMap<Identifier, RncPattern>, MacroError> {
    let mut definitions = Default::default();

    // TODO Include SVG and MathML schemas.
    for file in ["html5.rnc", "rdfa.rnc"] {
        load_schema(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src")
                .join("schema")
                .join("html5")
                .join(file),
            &mut definitions,
        )?;
    }

    Ok(definitions)
}

fn load_schema(
    path: &Path,
    definitions: &mut BTreeMap<Identifier, RncPattern>,
) -> Result<(), MacroError> {
    let schema = parse_schema(&read_to_string(path)?)?;

    // We do not use the declarations.
    // TODO Respect namespace declarations.

    match schema.body {
        SchemaBody::Grammar(grammar) => {
            load_grammar(
                &grammar,
                definitions,
                path.parent().ok_or(MacroError::NoParentDirectory)?,
            )?;
        }
        SchemaBody::Pattern(_) => return Err(MacroError::RncSyntax("top-level pattern")),
    }

    Ok(())
}

pub fn load_grammar(
    grammar: &Grammar,
    definitions: &mut BTreeMap<Identifier, RncPattern>,
    directory: &Path,
) -> Result<(), MacroError> {
    for content in &grammar.contents {
        match content {
            GrammarContent::Definition(definition) => {
                let name = definition.name.clone();
                let pattern = definition.pattern.clone();

                if let Some(combine) = definition.combine {
                    combine_patterns(
                        definitions.entry(name).or_insert(RncPattern::NotAllowed),
                        pattern,
                        combine,
                    );
                } else {
                    definitions.insert(name, pattern);
                }
            }
            GrammarContent::Div(grammar) => load_grammar(grammar, definitions, directory)?,
            GrammarContent::Include(include) => {
                load_schema(&directory.join(&include.uri), definitions)?;

                if let Some(grammar) = &include.grammar {
                    load_grammar(grammar, definitions, directory)?;
                }
            }
            GrammarContent::Annotation(_) | GrammarContent::Start { .. } => {}
        }
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
