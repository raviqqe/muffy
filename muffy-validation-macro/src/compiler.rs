use crate::{
    attribute::{AttributeSet, normalize_attributes},
    error::MacroError,
    name::{class_names, identifier_string},
    pattern::{ResolvedPattern, normalize_pattern},
};
use alloc::collections::BTreeMap;
use muffy_rnc::{Identifier, Pattern as RncPattern};

pub struct Compiler<'a> {
    definitions: &'a BTreeMap<Identifier, RncPattern>,
    cache: BTreeMap<Identifier, ResolvedPattern>,
}

impl<'a> Compiler<'a> {
    pub fn new(definitions: &'a BTreeMap<Identifier, RncPattern>) -> Self {
        Self {
            definitions,
            cache: Default::default(),
        }
    }

    pub fn compile(
        &mut self,
        pattern: &RncPattern,
    ) -> Result<Vec<(Vec<AttributeSet>, ResolvedPattern)>, MacroError> {
        normalize_pattern(&self.resolve(pattern)?)?
            .into_iter()
            .filter_map(|(attribute_pattern, content_pattern)| {
                match normalize_attributes(&attribute_pattern) {
                    Ok(attribute_sets)
                        if attribute_sets.is_empty()
                            || content_pattern == ResolvedPattern::NotAllowed =>
                    {
                        None
                    }
                    Ok(attribute_sets) => Some(Ok((attribute_sets, content_pattern))),
                    Err(error) => Some(Err(error)),
                }
            })
            .collect()
    }

    fn resolve(&mut self, pattern: &RncPattern) -> Result<ResolvedPattern, MacroError> {
        Ok(match pattern {
            RncPattern::Attribute { name_class, .. } => {
                let names = class_names(name_class, true);

                if names.is_empty() {
                    ResolvedPattern::NotAllowed
                } else {
                    ResolvedPattern::Attribute(names)
                }
            }
            RncPattern::Choice(patterns) => ResolvedPattern::choice(
                patterns
                    .iter()
                    .map(|pattern| self.resolve(pattern))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            RncPattern::Element { name_class, .. } => {
                let names = class_names(name_class, false);

                if names.is_empty() {
                    ResolvedPattern::NotAllowed
                } else {
                    ResolvedPattern::Element(names)
                }
            }
            RncPattern::Empty => ResolvedPattern::Empty,
            RncPattern::External(_) => return Err(MacroError::RncPattern("external")),
            RncPattern::Grammar(_) => return Err(MacroError::RncPattern("grammar")),
            RncPattern::Group(patterns) => ResolvedPattern::group(
                patterns
                    .iter()
                    .map(|pattern| self.resolve(pattern))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            RncPattern::Interleave(patterns) => ResolvedPattern::interleave(
                patterns
                    .iter()
                    .map(|pattern| self.resolve(pattern))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            RncPattern::Many0(pattern) => ResolvedPattern::many0(self.resolve(pattern)?),
            RncPattern::Many1(pattern) => ResolvedPattern::many1(self.resolve(pattern)?),
            RncPattern::NotAllowed => ResolvedPattern::NotAllowed,
            RncPattern::Optional(pattern) => ResolvedPattern::optional(self.resolve(pattern)?),
            RncPattern::Name(name) => {
                if let Some(pattern) = self.cache.get(&name.local) {
                    pattern.clone()
                } else if let Some(definition) = self.definitions.get(&name.local) {
                    let pattern = self.resolve(definition)?;
                    self.cache.insert(name.local.clone(), pattern.clone());
                    pattern
                } else {
                    return Err(MacroError::UndefinedReference(identifier_string(
                        &name.local,
                    )));
                }
            }
            // TODO Validate texts and attribute values against data and value patterns.
            RncPattern::Text
            | RncPattern::Data { .. }
            | RncPattern::List(_)
            | RncPattern::Value { .. } => ResolvedPattern::Text,
        })
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
