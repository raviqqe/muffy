use super::set::AttributeSet;
use crate::{error::MacroError, pattern::Pattern};
use alloc::collections::BTreeSet;

pub fn normalize_attributes(pattern: &Pattern) -> Result<Vec<AttributeSet>, MacroError> {
    let mut sets = normalize(pattern)?;

    sets.sort();
    sets.dedup();

    Ok(sets)
}

fn normalize(pattern: &Pattern) -> Result<Vec<AttributeSet>, MacroError> {
    Ok(match pattern {
        Pattern::Empty => vec![Default::default()],
        Pattern::NotAllowed => vec![],
        Pattern::Attribute(names) => name_choice(names),
        Pattern::Choice(patterns) => patterns
            .iter()
            .map(normalize)
            .collect::<Result<Vec<_>, _>>()?
            .concat(),
        Pattern::Group(patterns) | Pattern::Interleave(patterns) => {
            let mut sets = vec![AttributeSet::default()];

            for pattern in patterns {
                let others = normalize(pattern)?;

                sets = sets
                    .iter()
                    .flat_map(|set| others.iter().map(|other| set.merge(other)))
                    .collect();
            }

            sets
        }
        Pattern::Optional(pattern) | Pattern::Many0(pattern) => optional(normalize(pattern)?),
        Pattern::Many1(pattern) => normalize(pattern)?,
        Pattern::Element(_) | Pattern::Text => {
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

    fn attribute(name: &str) -> Pattern {
        Pattern::Attribute([name.into()].into())
    }

    fn set(required: &[&str], optional: &[&str]) -> AttributeSet {
        AttributeSet {
            required: required.iter().copied().map(Into::into).collect(),
            optional: optional.iter().copied().map(Into::into).collect(),
        }
    }

    #[test]
    fn normalize_empty() {
        assert_eq!(
            normalize_attributes(&Pattern::Empty).unwrap(),
            vec![AttributeSet::default()]
        );
    }

    #[test]
    fn normalize_not_allowed() {
        assert_eq!(normalize_attributes(&Pattern::NotAllowed).unwrap(), vec![]);
    }

    #[test]
    fn normalize_required_attribute() {
        assert_eq!(
            normalize_attributes(&attribute("foo")).unwrap(),
            vec![set(&["foo"], &[])]
        );
    }

    #[test]
    fn normalize_optional_attribute() {
        assert_eq!(
            normalize_attributes(&Pattern::optional(attribute("foo"))).unwrap(),
            vec![set(&[], &["foo"])]
        );
    }

    #[test]
    fn normalize_interleave_of_optional_attributes() {
        assert_eq!(
            normalize_attributes(&Pattern::interleave([
                Pattern::optional(attribute("foo")),
                Pattern::optional(attribute("bar")),
            ]))
            .unwrap(),
            vec![set(&[], &["bar", "foo"])]
        );
    }

    #[test]
    fn normalize_choice_of_attributes() {
        assert_eq!(
            normalize_attributes(&Pattern::choice([attribute("foo"), attribute("bar")])).unwrap(),
            vec![set(&["bar"], &[]), set(&["foo"], &[])]
        );
    }

    #[test]
    fn normalize_group_product() {
        assert_eq!(
            normalize_attributes(&Pattern::group([
                Pattern::choice([attribute("foo"), attribute("bar")]),
                attribute("baz"),
            ]))
            .unwrap(),
            vec![set(&["bar", "baz"], &[]), set(&["baz", "foo"], &[])]
        );
    }

    #[test]
    fn normalize_exclusive_attribute_pair() {
        assert_eq!(
            normalize_attributes(&Pattern::choice([
                Pattern::group([attribute("foo"), Pattern::optional(attribute("bar"))]),
                Pattern::group([Pattern::optional(attribute("foo")), attribute("bar")]),
            ]))
            .unwrap(),
            vec![set(&["bar"], &["foo"]), set(&["foo"], &["bar"])]
        );
    }

    #[test]
    fn normalize_at_least_one_attribute() {
        assert_eq!(
            normalize_attributes(&Pattern::many1(attribute("foo"))).unwrap(),
            vec![set(&["foo"], &[])]
        );
    }

    #[test]
    fn normalize_alternative_attribute_names() {
        assert_eq!(
            normalize_attributes(&Pattern::Attribute(["foo".into(), "bar".into()].into())).unwrap(),
            vec![set(&["bar"], &["foo"]), set(&["foo"], &["bar"])]
        );
    }

    #[test]
    fn normalize_optional_alternative_attribute_names() {
        assert_eq!(
            normalize_attributes(&Pattern::optional(Pattern::Attribute(
                ["foo".into(), "bar".into()].into()
            )))
            .unwrap(),
            vec![set(&[], &["bar", "foo"])]
        );
    }

    #[test]
    fn normalize_optional_choice_of_attributes() {
        // Exactly one of the exclusive attributes may be present.
        assert_eq!(
            normalize_attributes(&Pattern::optional(Pattern::choice([
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
            normalize_attributes(&Pattern::Element(["foo".into()].into())),
            Err(MacroError::RncPattern(_))
        ));
    }
}
