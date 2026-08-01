use super::{ResolvedPattern, compile::Compiler, element_class_names, identifier_string};
use crate::error::MacroError;
use alloc::collections::BTreeSet;
use muffy_rnc::{NameClass, Pattern};

impl Compiler<'_> {
    pub(super) fn resolve(&mut self, pattern: &Pattern) -> Result<ResolvedPattern, MacroError> {
        Ok(match pattern {
            Pattern::Attribute { name_class, .. } => {
                let names = attribute_class_names(name_class);

                if names.is_empty() {
                    ResolvedPattern::NotAllowed
                } else {
                    ResolvedPattern::Attribute(names)
                }
            }
            Pattern::Choice(patterns) => ResolvedPattern::choice(
                patterns
                    .iter()
                    .map(|pattern| self.resolve(pattern))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            Pattern::Element { name_class, .. } => {
                let names = element_class_names(name_class);

                if names.is_empty() {
                    ResolvedPattern::NotAllowed
                } else {
                    ResolvedPattern::Element(names)
                }
            }
            Pattern::Empty => ResolvedPattern::Empty,
            Pattern::External(_) => return Err(MacroError::RncPattern("external")),
            Pattern::Grammar(_) => return Err(MacroError::RncPattern("grammar")),
            Pattern::Group(patterns) => ResolvedPattern::group(
                patterns
                    .iter()
                    .map(|pattern| self.resolve(pattern))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            Pattern::Interleave(patterns) => ResolvedPattern::interleave(
                patterns
                    .iter()
                    .map(|pattern| self.resolve(pattern))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            Pattern::Many0(pattern) => ResolvedPattern::many0(self.resolve(pattern)?),
            Pattern::Many1(pattern) => ResolvedPattern::many1(self.resolve(pattern)?),
            Pattern::NotAllowed => ResolvedPattern::NotAllowed,
            Pattern::Optional(pattern) => ResolvedPattern::optional(self.resolve(pattern)?),
            Pattern::Name(name) => {
                if let Some(resolved) = self.cache.get(&name.local) {
                    resolved.clone()
                } else if let Some(definition) = self.definitions.get(&name.local) {
                    let resolved = self.resolve(definition)?;
                    self.cache.insert(name.local.clone(), resolved.clone());
                    resolved
                } else {
                    return Err(MacroError::UndefinedReference(identifier_string(
                        &name.local,
                    )));
                }
            }
            // TODO Validate texts and attribute values against data and value patterns.
            Pattern::Text | Pattern::Data { .. } | Pattern::List(_) | Pattern::Value { .. } => {
                ResolvedPattern::Text
            }
        })
    }
}
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
        // TODO Support wildcard name classes. (e.g. arbitrary attributes of embed elements)
        NameClass::AnyName | NameClass::Except { .. } | NameClass::NamespaceName(_) => {
            Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::collections::BTreeMap;
    use muffy_rnc::{Identifier, SchemaBody, parse_schema};
    use pretty_assertions::assert_eq;
    use std::path::Path;

    fn attribute(name: &str) -> ResolvedPattern {
        ResolvedPattern::Attribute([name.into()].into())
    }

    fn resolve(source: &str) -> Result<ResolvedPattern, MacroError> {
        let SchemaBody::Grammar(grammar) = parse_schema(source).unwrap().body else {
            panic!("grammar expected");
        };
        let mut definitions = BTreeMap::new();

        crate::load_grammar(&grammar, &mut definitions, Path::new(".")).unwrap();

        Compiler::new(&definitions).resolve(
            &definitions[&Identifier {
                component: "root".into(),
                sub_components: vec![],
            }],
        )
    }

    #[test]
    fn resolve_attribute() {
        assert_eq!(
            resolve("root = attribute foo { text }").unwrap(),
            attribute("foo")
        );
    }

    #[test]
    fn resolve_prefixed_attribute_names() {
        assert_eq!(
            resolve("root = attribute xml:lang { text }").unwrap(),
            ResolvedPattern::Attribute(["lang".into(), "xml:lang".into()].into())
        );
    }

    #[test]
    fn resolve_reference() {
        assert_eq!(
            resolve("root = foo\nfoo = attribute bar { text }").unwrap(),
            attribute("bar")
        );
    }

    #[test]
    fn evaluate_empty_flag() {
        assert_eq!(
            resolve("root = attribute foo { text } & gate\ngate = empty").unwrap(),
            attribute("foo")
        );
    }

    #[test]
    fn evaluate_not_allowed_flag() {
        assert_eq!(
            resolve("root = attribute foo { text } & gate\ngate = notAllowed").unwrap(),
            ResolvedPattern::NotAllowed
        );
    }

    #[test]
    fn drop_wildcard_attribute_repetition() {
        assert_eq!(
            resolve("root = attribute * { text }*").unwrap(),
            ResolvedPattern::Empty
        );
    }

    #[test]
    fn combine_choice_of_element_names() {
        assert_eq!(
            resolve("root = element (foo | bar) { empty }").unwrap(),
            ResolvedPattern::Element(["bar".into(), "foo".into()].into())
        );
    }

    #[test]
    fn fail_on_undefined_reference() {
        assert!(matches!(
            resolve("root = foo"),
            Err(MacroError::UndefinedReference(_))
        ));
    }
}
