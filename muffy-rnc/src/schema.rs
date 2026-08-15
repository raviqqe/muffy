mod error;

pub use self::error::SchemaError;
use crate::ast::{Combine, Definition, Identifier, IncludeContent, Pattern};
use alloc::collections::{BTreeMap, BTreeSet};
use core::mem::replace;

/// A definition set.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DefinitionSet {
    definitions: BTreeMap<Identifier, (Option<Combine>, Pattern)>,
}

impl DefinitionSet {
    /// Defines a pattern, merging it into an existing definition by its
    /// combine operator.
    pub fn define(&mut self, definition: &Definition) -> Result<(), SchemaError> {
        let pattern = definition.pattern.clone();

        if let Some(combine) = definition.combine
            && let Some((operator, existing)) = self.definitions.get_mut(&definition.name)
        {
            if let Some(operator) = *operator
                && operator != combine
            {
                return Err(SchemaError::CombineConflict(definition.name.clone()));
            }

            combine_patterns(existing, pattern, combine);
            *operator = Some(combine);
        } else if definition.combine.is_none()
            && let Some((Some(operator), existing)) = self.definitions.get_mut(&definition.name)
        {
            combine_patterns(existing, pattern, *operator);
        } else {
            self.definitions
                .insert(definition.name.clone(), (definition.combine, pattern));
        }

        Ok(())
    }

    /// Converts a definition set into patterns.
    pub fn into_patterns(self) -> BTreeMap<Identifier, Pattern> {
        self.definitions
            .into_iter()
            .map(|(name, (_, pattern))| (name, pattern))
            .collect()
    }
}

/// Collects names defined in include contents, descending into div blocks.
pub fn defined_names(contents: &[IncludeContent]) -> BTreeSet<Identifier> {
    contents
        .iter()
        .flat_map(|content| match content {
            IncludeContent::Definition(definition) => [definition.name.clone()].into(),
            IncludeContent::Div(contents) => defined_names(contents),
            IncludeContent::Annotation(_) | IncludeContent::Start(_) => Default::default(),
        })
        .collect()
}

fn combine_patterns(existing: &mut Pattern, new: Pattern, combine: Combine) {
    match combine {
        Combine::Choice => match existing {
            Pattern::Choice(choices) => choices.push(new),
            Pattern::NotAllowed => *existing = new,
            Pattern::Attribute { .. }
            | Pattern::Data { .. }
            | Pattern::Element { .. }
            | Pattern::Empty
            | Pattern::External(_)
            | Pattern::Grammar(_)
            | Pattern::Group(_)
            | Pattern::Interleave(_)
            | Pattern::List(_)
            | Pattern::Many0(_)
            | Pattern::Many1(_)
            | Pattern::Name(_)
            | Pattern::Optional(_)
            | Pattern::Text
            | Pattern::Value { .. } => {
                let old = replace(existing, Pattern::Choice(vec![]));

                if let Pattern::Choice(choices) = existing {
                    choices.push(old);
                    choices.push(new);
                }
            }
        },
        Combine::Interleave => match existing {
            Pattern::Interleave(patterns) => patterns.push(new),
            Pattern::NotAllowed => *existing = new,
            Pattern::Attribute { .. }
            | Pattern::Choice(_)
            | Pattern::Data { .. }
            | Pattern::Element { .. }
            | Pattern::Empty
            | Pattern::External(_)
            | Pattern::Grammar(_)
            | Pattern::Group(_)
            | Pattern::List(_)
            | Pattern::Many0(_)
            | Pattern::Many1(_)
            | Pattern::Name(_)
            | Pattern::Optional(_)
            | Pattern::Text
            | Pattern::Value { .. } => {
                let old = replace(existing, Pattern::Interleave(vec![]));

                if let Pattern::Interleave(patterns) = existing {
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
    use crate::{GrammarContent, SchemaBody, parse_schema};
    use pretty_assertions::assert_eq;

    fn load(source: &str, definitions: &mut DefinitionSet) -> Result<(), SchemaError> {
        let SchemaBody::Grammar(grammar) = parse_schema(source).unwrap().body else {
            panic!("grammar expected");
        };

        for content in &grammar.contents {
            let GrammarContent::Definition(definition) = content else {
                panic!("definition expected");
            };

            definitions.define(definition)?;
        }

        Ok(())
    }

    fn patterns(definitions: DefinitionSet) -> Vec<Pattern> {
        definitions.into_patterns().into_values().collect()
    }

    fn attribute(name: &str) -> Pattern {
        let mut definitions = DefinitionSet::default();

        load(
            &format!("root = attribute {name} {{ text }}"),
            &mut definitions,
        )
        .unwrap();

        patterns(definitions).remove(0)
    }

    #[test]
    fn merge_chosen_definitions() {
        let mut definitions = DefinitionSet::default();

        load(
            "root |= attribute foo { text }\nroot |= attribute bar { text }",
            &mut definitions,
        )
        .unwrap();

        assert_eq!(
            patterns(definitions),
            vec![Pattern::Choice(vec![attribute("foo"), attribute("bar")])]
        );
    }

    #[test]
    fn merge_interleaved_definitions() {
        let mut definitions = DefinitionSet::default();

        load(
            "root &= attribute foo { text }\nroot &= attribute bar { text }",
            &mut definitions,
        )
        .unwrap();

        assert_eq!(
            patterns(definitions),
            vec![Pattern::Interleave(vec![
                attribute("foo"),
                attribute("bar")
            ])]
        );
    }

    #[test]
    fn merge_plain_definition_before_combined_definition() {
        let mut definitions = DefinitionSet::default();

        load(
            "root = attribute foo { text }\nroot &= attribute bar { text }",
            &mut definitions,
        )
        .unwrap();

        assert_eq!(
            patterns(definitions),
            vec![Pattern::Interleave(vec![
                attribute("foo"),
                attribute("bar")
            ])]
        );
    }

    #[test]
    fn merge_plain_definition_after_combined_definition() {
        let mut definitions = DefinitionSet::default();

        load(
            "root &= attribute foo { text }\nroot = attribute bar { text }",
            &mut definitions,
        )
        .unwrap();

        assert_eq!(
            patterns(definitions),
            vec![Pattern::Interleave(vec![
                attribute("foo"),
                attribute("bar")
            ])]
        );
    }

    #[test]
    fn overwrite_duplicate_plain_definition() {
        let mut definitions = DefinitionSet::default();

        load("root = empty\nroot = notAllowed", &mut definitions).unwrap();

        assert_eq!(patterns(definitions), vec![Pattern::NotAllowed]);
    }

    fn include_contents(source: &str) -> Vec<IncludeContent> {
        let SchemaBody::Grammar(grammar) =
            parse_schema(&format!("include \"baz.rnc\" {{ {source} }}"))
                .unwrap()
                .body
        else {
            panic!("grammar expected");
        };
        let GrammarContent::Include(include) = &grammar.contents[0] else {
            panic!("include expected");
        };

        include.contents.clone()
    }

    #[test]
    fn collect_defined_names() {
        assert_eq!(
            defined_names(&include_contents("foo = empty\nbar = empty\nstart = foo"))
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            vec!["bar", "foo"]
        );
    }

    #[test]
    fn collect_defined_names_in_div() {
        assert_eq!(
            defined_names(&include_contents("div { foo = empty }"))
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            vec!["foo"]
        );
    }

    #[test]
    fn fail_on_conflicting_combine_operators() {
        assert!(matches!(
            load(
                "root |= attribute foo { text }\nroot &= attribute bar { text }",
                &mut DefinitionSet::default(),
            ),
            Err(SchemaError::CombineConflict(_))
        ));
    }

    #[test]
    fn fail_on_combine_operator_conflicting_with_merged_plain_definition() {
        assert!(matches!(
            load(
                "root |= attribute foo { text }\nroot = empty\nroot &= attribute bar { text }",
                &mut DefinitionSet::default(),
            ),
            Err(SchemaError::CombineConflict(_))
        ));
    }
}
