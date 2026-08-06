use crate::{
    attribute::{AttributeSet, normalize_attributes},
    error::MacroError,
    name::class_names,
    pattern::{Pattern, normalize_pattern},
};
use alloc::collections::BTreeMap;
use muffy_rnc::{Identifier, Pattern as RncPattern};

pub struct Compiler<'a> {
    definitions: &'a BTreeMap<Identifier, RncPattern>,
    cache: BTreeMap<Identifier, Pattern>,
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
    ) -> Result<Vec<(Vec<AttributeSet>, Pattern)>, MacroError> {
        normalize_pattern(&self.resolve(pattern)?)?
            .into_iter()
            .filter_map(|(attribute_pattern, content_pattern)| {
                match normalize_attributes(&attribute_pattern) {
                    Ok(attribute_sets)
                        if attribute_sets.is_empty() || content_pattern == Pattern::NotAllowed =>
                    {
                        None
                    }
                    Ok(attribute_sets) => Some(Ok((attribute_sets, content_pattern))),
                    Err(error) => Some(Err(error)),
                }
            })
            .collect()
    }

    fn resolve(&mut self, pattern: &RncPattern) -> Result<Pattern, MacroError> {
        Ok(match pattern {
            RncPattern::Attribute { name_class, .. } => {
                let names = class_names(name_class, true);

                if names.is_empty() {
                    Pattern::NotAllowed
                } else {
                    Pattern::Attribute(names)
                }
            }
            RncPattern::Choice(patterns) => Pattern::choice(
                patterns
                    .iter()
                    .map(|pattern| self.resolve(pattern))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            RncPattern::Element { name_class, .. } => {
                let names = class_names(name_class, false);

                if names.is_empty() {
                    Pattern::NotAllowed
                } else {
                    Pattern::Element(names)
                }
            }
            RncPattern::Empty => Pattern::Empty,
            RncPattern::External(_) => return Err(MacroError::RncPattern("external")),
            RncPattern::Grammar(_) => return Err(MacroError::RncPattern("grammar")),
            RncPattern::Group(patterns) => Pattern::group(
                patterns
                    .iter()
                    .map(|pattern| self.resolve(pattern))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            RncPattern::Interleave(patterns) => Pattern::interleave(
                patterns
                    .iter()
                    .map(|pattern| self.resolve(pattern))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            RncPattern::Many0(pattern) => Pattern::many0(self.resolve(pattern)?),
            RncPattern::Many1(pattern) => Pattern::many1(self.resolve(pattern)?),
            RncPattern::NotAllowed => Pattern::NotAllowed,
            RncPattern::Optional(pattern) => Pattern::optional(self.resolve(pattern)?),
            RncPattern::Name(name) => {
                if let Some(pattern) = self.cache.get(&name.local) {
                    pattern.clone()
                } else {
                    let Some(definition) = self.definitions.get(&name.local) else {
                        return Err(MacroError::UndefinedReference(name.local.to_string()));
                    };

                    let pattern = self.resolve(definition)?;
                    self.cache.insert(name.local.clone(), pattern.clone());
                    pattern
                }
            }
            // TODO Validate texts and attribute values against data and value patterns.
            RncPattern::Text
            | RncPattern::Data { .. }
            | RncPattern::List(_)
            | RncPattern::Value { .. } => Pattern::Text,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definition::load_grammar;
    use muffy_rnc::{DefinitionSet, Identifier, SchemaBody, parse_schema};
    use pretty_assertions::assert_eq;
    use std::path::Path;

    fn attribute(name: &str) -> Pattern {
        Pattern::Attribute([name.into()].into())
    }

    fn resolve(source: &str) -> Result<Pattern, MacroError> {
        let SchemaBody::Grammar(grammar) = parse_schema(source).unwrap().body else {
            panic!("grammar expected");
        };
        let mut definitions = DefinitionSet::default();

        load_grammar(&grammar, Path::new("."), false, &mut definitions).unwrap();

        let definitions = definitions.into_patterns();

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
            Pattern::Attribute(["xml:lang".into()].into())
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
            Pattern::NotAllowed
        );
    }

    #[test]
    fn drop_wildcard_attribute_repetition() {
        assert_eq!(
            resolve("root = attribute * { text }*").unwrap(),
            Pattern::Empty
        );
    }

    #[test]
    fn combine_choice_of_element_names() {
        assert_eq!(
            resolve("root = element (foo | bar) { empty }").unwrap(),
            Pattern::Element(["bar".into(), "foo".into()].into())
        );
    }

    #[test]
    fn merge_plain_definition_before_interleaved_definition() {
        assert_eq!(
            resolve("root = attribute foo { text }\nroot &= attribute bar { text }").unwrap(),
            Pattern::interleave([attribute("foo"), attribute("bar")])
        );
    }

    #[test]
    fn merge_plain_definition_after_interleaved_definition() {
        assert_eq!(
            resolve("root &= attribute foo { text }\nroot = attribute bar { text }").unwrap(),
            Pattern::interleave([attribute("foo"), attribute("bar")])
        );
    }

    #[test]
    fn merge_plain_definition_after_chosen_definition() {
        assert_eq!(
            resolve("root |= attribute foo { text }\nroot = attribute bar { text }").unwrap(),
            Pattern::choice([attribute("foo"), attribute("bar")])
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
