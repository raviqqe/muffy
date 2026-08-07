use crate::error::MacroError;
use alloc::collections::BTreeMap;
use muffy_rnc::{
    DefinitionSet, Grammar, GrammarContent, Identifier, Pattern as RncPattern, SchemaBody,
    parse_schema,
};
use std::{fs::read_to_string, path::Path};

pub fn load_definitions(files: &[&str]) -> Result<BTreeMap<Identifier, RncPattern>, MacroError> {
    let mut definitions = DefinitionSet::default();

    for file in files {
        load_schema(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("src").join(file),
            &mut definitions,
        )?;
    }

    Ok(definitions.into_patterns())
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
            GrammarContent::Definition(definition) => definitions.define(definition, replace)?,
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
