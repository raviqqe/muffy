use super::set::AttributeSet;
use crate::{error::MacroError, pattern::CompiledPattern};
use alloc::collections::BTreeSet;

pub fn compile_attributes(pattern: &CompiledPattern) -> Result<Vec<AttributeSet>, MacroError> {
    let mut sets = compile(pattern)?;

    sets.sort();
    sets.dedup();

    Ok(sets)
}

fn compile(pattern: &CompiledPattern) -> Result<Vec<AttributeSet>, MacroError> {
    Ok(match pattern {
        CompiledPattern::Empty => vec![Default::default()],
        CompiledPattern::NotAllowed => vec![],
        CompiledPattern::Attribute(names) => name_choice(names),
        CompiledPattern::Choice(patterns) => patterns
            .iter()
            .map(compile)
            .collect::<Result<Vec<_>, _>>()?
            .concat(),
        CompiledPattern::Group(patterns) | CompiledPattern::Interleave(patterns) => {
            let mut sets = vec![AttributeSet::default()];

            for pattern in patterns {
                let others = compile(pattern)?;

                sets = sets
                    .iter()
                    .flat_map(|set| others.iter().map(|other| set.merge(other)))
                    .collect();
            }

            sets
        }
        CompiledPattern::Optional(pattern) | CompiledPattern::Many0(pattern) => {
            optional(compile(pattern)?)
        }
        CompiledPattern::Many1(pattern) => compile(pattern)?,
        CompiledPattern::Element(_) | CompiledPattern::Text => {
            return Err(MacroError::RncPattern("content in attribute pattern"));
        }
    })
}

fn optional(mut sets: Vec<AttributeSet>) -> Vec<AttributeSet> {
    sets.sort();
    sets.dedup();

    let names = sets
        .iter()
        .flat_map(|set| set.required.iter().chain(&set.optional))
        .cloned()
        .collect::<BTreeSet<_>>();

    if sets == name_choice(&names) {
        vec![AttributeSet {
            required: Default::default(),
            optional: names,
        }]
    } else if sets.iter().any(|set| set.required.is_empty()) {
        sets
    } else {
        [AttributeSet::default()].into_iter().chain(sets).collect()
    }
}

fn name_choice(names: &BTreeSet<String>) -> Vec<AttributeSet> {
    names
        .iter()
        .map(|name| AttributeSet {
            required: [name.clone()].into(),
            optional: names
                .iter()
                .filter(|other| *other != name)
                .cloned()
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn attribute(name: &str) -> CompiledPattern {
        CompiledPattern::Attribute([name.into()].into())
    }

    fn set(required: &[&str], optional: &[&str]) -> AttributeSet {
        AttributeSet {
            required: required.iter().copied().map(Into::into).collect(),
            optional: optional.iter().copied().map(Into::into).collect(),
        }
    }

    #[test]
    fn compile_empty() {
        assert_eq!(
            compile_attributes(&CompiledPattern::Empty).unwrap(),
            vec![AttributeSet::default()]
        );
    }

    #[test]
    fn compile_not_allowed() {
        assert_eq!(
            compile_attributes(&CompiledPattern::NotAllowed).unwrap(),
            vec![]
        );
    }

    #[test]
    fn compile_required_attribute() {
        assert_eq!(
            compile_attributes(&attribute("foo")).unwrap(),
            vec![set(&["foo"], &[])]
        );
    }

    #[test]
    fn compile_optional_attribute() {
        assert_eq!(
            compile_attributes(&CompiledPattern::optional(attribute("foo"))).unwrap(),
            vec![set(&[], &["foo"])]
        );
    }

    #[test]
    fn compile_interleave_of_optional_attributes() {
        assert_eq!(
            compile_attributes(&CompiledPattern::interleave([
                CompiledPattern::optional(attribute("foo")),
                CompiledPattern::optional(attribute("bar")),
            ]))
            .unwrap(),
            vec![set(&[], &["bar", "foo"])]
        );
    }

    #[test]
    fn compile_choice_of_attributes() {
        assert_eq!(
            compile_attributes(&CompiledPattern::choice([
                attribute("foo"),
                attribute("bar")
            ]))
            .unwrap(),
            vec![set(&["bar"], &[]), set(&["foo"], &[])]
        );
    }

    #[test]
    fn compile_group_product() {
        assert_eq!(
            compile_attributes(&CompiledPattern::group([
                CompiledPattern::choice([attribute("foo"), attribute("bar")]),
                attribute("baz"),
            ]))
            .unwrap(),
            vec![set(&["bar", "baz"], &[]), set(&["baz", "foo"], &[])]
        );
    }

    #[test]
    fn compile_exclusive_attribute_pair() {
        assert_eq!(
            compile_attributes(&CompiledPattern::choice([
                CompiledPattern::group([
                    attribute("foo"),
                    CompiledPattern::optional(attribute("bar"))
                ]),
                CompiledPattern::group([
                    CompiledPattern::optional(attribute("foo")),
                    attribute("bar")
                ]),
            ]))
            .unwrap(),
            vec![set(&["bar"], &["foo"]), set(&["foo"], &["bar"])]
        );
    }

    #[test]
    fn compile_at_least_one_attribute() {
        assert_eq!(
            compile_attributes(&CompiledPattern::many1(attribute("foo"))).unwrap(),
            vec![set(&["foo"], &[])]
        );
    }

    #[test]
    fn compile_alternative_attribute_names() {
        assert_eq!(
            compile_attributes(&CompiledPattern::Attribute(
                ["foo".into(), "bar".into()].into()
            ))
            .unwrap(),
            vec![set(&["bar"], &["foo"]), set(&["foo"], &["bar"])]
        );
    }

    #[test]
    fn compile_optional_alternative_attribute_names() {
        assert_eq!(
            compile_attributes(&CompiledPattern::optional(CompiledPattern::Attribute(
                ["foo".into(), "bar".into()].into()
            )))
            .unwrap(),
            vec![set(&[], &["bar", "foo"])]
        );
    }

    #[test]
    fn compile_optional_choice_of_attributes() {
        // Exactly one of the exclusive attributes may be present.
        assert_eq!(
            compile_attributes(&CompiledPattern::optional(CompiledPattern::choice([
                attribute("foo"),
                attribute("bar")
            ])))
            .unwrap(),
            vec![set(&[], &[]), set(&["bar"], &[]), set(&["foo"], &[])]
        );
    }

    #[test]
    fn fail_on_element() {
        assert!(matches!(
            compile_attributes(&CompiledPattern::Element(["foo".into()].into())),
            Err(MacroError::RncPattern(_))
        ));
    }
}
