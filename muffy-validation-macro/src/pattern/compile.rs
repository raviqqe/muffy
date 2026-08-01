use super::{CompiledPattern, class_names, identifier_string};
use crate::error::MacroError;
use alloc::collections::{BTreeMap, BTreeSet};
use muffy_rnc::{Identifier, NameClass, Pattern};

// HTML parsers match a prefixed schema name (e.g. `xml:lang`) against its bare
// local name while the literal prefixed spelling is also conforming, so an
// attribute matches both names.
fn attribute_class_names(name_class: &NameClass) -> BTreeSet<String> {
    match name_class {
        NameClass::Name(name) => {
            let local = identifier_string(&name.local);

            if let Some(prefix) = &name.prefix {
                [format!("{}:{local}", identifier_string(prefix)), local].into()
            } else {
                [local].into()
            }
        }
        NameClass::Choice(classes) => classes.iter().flat_map(attribute_class_names).collect(),
        // TODO Support wildcard name classes (e.g. arbitrary attributes of
        // embed elements).
        NameClass::AnyName | NameClass::Except { .. } | NameClass::NamespaceName(_) => {
            Default::default()
        }
    }
}

pub fn compile_pattern(
    pattern: &Pattern,
    definitions: &BTreeMap<Identifier, Pattern>,
    cache: &mut BTreeMap<Identifier, CompiledPattern>,
) -> Result<CompiledPattern, MacroError> {
    compile_pattern_with_stack(pattern, definitions, cache, &mut vec![])
}

fn compile_pattern_with_stack(
    pattern: &Pattern,
    definitions: &BTreeMap<Identifier, Pattern>,
    cache: &mut BTreeMap<Identifier, CompiledPattern>,
    stack: &mut Vec<Identifier>,
) -> Result<CompiledPattern, MacroError> {
    Ok(match pattern {
        Pattern::Attribute { name_class, .. } => {
            let names = attribute_class_names(name_class);

            if names.is_empty() {
                CompiledPattern::NotAllowed
            } else {
                CompiledPattern::Attribute(names)
            }
        }
        Pattern::Element { name_class, .. } => {
            let names = class_names(name_class);

            if names.is_empty() {
                CompiledPattern::NotAllowed
            } else {
                CompiledPattern::Element(names)
            }
        }
        Pattern::Choice(patterns) => CompiledPattern::choice(
            patterns
                .iter()
                .map(|pattern| compile_pattern_with_stack(pattern, definitions, cache, stack))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        Pattern::Group(patterns) => CompiledPattern::group(
            patterns
                .iter()
                .map(|pattern| compile_pattern_with_stack(pattern, definitions, cache, stack))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        Pattern::Interleave(patterns) => CompiledPattern::interleave(
            patterns
                .iter()
                .map(|pattern| compile_pattern_with_stack(pattern, definitions, cache, stack))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        Pattern::Many0(pattern) => CompiledPattern::many0(compile_pattern_with_stack(
            pattern,
            definitions,
            cache,
            stack,
        )?),
        Pattern::Many1(pattern) => CompiledPattern::many1(compile_pattern_with_stack(
            pattern,
            definitions,
            cache,
            stack,
        )?),
        Pattern::Optional(pattern) => CompiledPattern::optional(compile_pattern_with_stack(
            pattern,
            definitions,
            cache,
            stack,
        )?),
        Pattern::Empty => CompiledPattern::Empty,
        Pattern::NotAllowed => CompiledPattern::NotAllowed,
        // TODO Validate texts and attribute values against data and value
        // patterns.
        Pattern::Text | Pattern::Data { .. } | Pattern::List(_) | Pattern::Value { .. } => {
            CompiledPattern::Text
        }
        Pattern::External(_) => return Err(MacroError::RncPattern("external")),
        Pattern::Grammar(_) => return Err(MacroError::RncPattern("grammar")),
        Pattern::Name(name) => {
            if let Some(compiled) = cache.get(&name.local) {
                compiled.clone()
            } else if stack.contains(&name.local) {
                return Err(MacroError::CircularReference(identifier_string(
                    &name.local,
                )));
            } else if let Some(definition) = definitions.get(&name.local) {
                stack.push(name.local.clone());
                let compiled = compile_pattern_with_stack(definition, definitions, cache, stack)?;
                stack.pop();
                cache.insert(name.local.clone(), compiled.clone());
                compiled
            } else {
                return Err(MacroError::UndefinedReference(identifier_string(
                    &name.local,
                )));
            }
        }
    })
}

/// Splits a compiled element pattern into alternatives of attribute and
/// content patterns whose interleaved union is equivalent to the original.

#[cfg(test)]
mod tests {
    use super::*;
    use muffy_rnc::{SchemaBody, parse_schema};
    use pretty_assertions::assert_eq;
    use std::path::Path;

    fn attribute(name: &str) -> CompiledPattern {
        CompiledPattern::Attribute([name.into()].into())
    }

    fn compile(source: &str) -> Result<CompiledPattern, MacroError> {
        let SchemaBody::Grammar(grammar) = parse_schema(source).unwrap().body else {
            panic!("grammar expected");
        };
        let mut definitions = BTreeMap::new();

        crate::load_grammar(&grammar, &mut definitions, Path::new(".")).unwrap();

        compile_pattern(
            &definitions[&Identifier {
                component: "root".into(),
                sub_components: vec![],
            }],
            &definitions,
            &mut Default::default(),
        )
    }

    #[test]
    fn compile_attribute() {
        assert_eq!(
            compile("root = attribute foo { text }").unwrap(),
            attribute("foo")
        );
    }

    #[test]
    fn compile_prefixed_attribute_names() {
        assert_eq!(
            compile("root = attribute xml:lang { text }").unwrap(),
            CompiledPattern::Attribute(["lang".into(), "xml:lang".into()].into())
        );
    }

    #[test]
    fn resolve_reference() {
        assert_eq!(
            compile("root = foo\nfoo = attribute bar { text }").unwrap(),
            attribute("bar")
        );
    }

    #[test]
    fn evaluate_empty_flag() {
        assert_eq!(
            compile("root = attribute foo { text } & gate\ngate = empty").unwrap(),
            attribute("foo")
        );
    }

    #[test]
    fn evaluate_not_allowed_flag() {
        assert_eq!(
            compile("root = attribute foo { text } & gate\ngate = notAllowed").unwrap(),
            CompiledPattern::NotAllowed
        );
    }

    #[test]
    fn drop_wildcard_attribute_repetition() {
        assert_eq!(
            compile("root = attribute * { text }*").unwrap(),
            CompiledPattern::Empty
        );
    }

    #[test]
    fn combine_choice_of_element_names() {
        assert_eq!(
            compile("root = element (foo | bar) { empty }").unwrap(),
            CompiledPattern::Element(["bar".into(), "foo".into()].into())
        );
    }

    #[test]
    fn fail_on_undefined_reference() {
        assert!(matches!(
            compile("root = foo"),
            Err(MacroError::UndefinedReference(_))
        ));
    }

    #[test]
    fn fail_on_circular_reference() {
        assert!(matches!(
            compile("root = foo\nfoo = (foo)"),
            Err(MacroError::CircularReference(_))
        ));
    }
}
