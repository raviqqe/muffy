use super::set::{AttributeSet, merge_attributes};
use crate::{error::MacroError, pattern::Pattern, value::Value};
use alloc::collections::BTreeMap;

pub fn normalize_attributes(pattern: &Pattern) -> Result<Vec<AttributeSet>, MacroError> {
    Ok(merge_sets(normalize(pattern)?))
}

fn normalize(pattern: &Pattern) -> Result<Vec<AttributeSet>, MacroError> {
    Ok(match pattern {
        Pattern::Empty => vec![Default::default()],
        Pattern::NotAllowed => vec![],
        Pattern::Attribute(names, value) => name_choice(
            &names
                .iter()
                .map(|name| (name.clone(), value.clone()))
                .collect(),
        ),
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

fn optional(sets: Vec<AttributeSet>) -> Vec<AttributeSet> {
    let sets = merge_sets(sets);

    let names = sets.iter().fold(BTreeMap::new(), |names, set| {
        merge_attributes(&merge_attributes(&names, &set.required), &set.optional)
    });

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

// Sets with the same attribute names are merged by unioning their value
// schemas, over-approximating value combinations across alternatives.
fn merge_sets(sets: Vec<AttributeSet>) -> Vec<AttributeSet> {
    let mut merged = BTreeMap::<_, AttributeSet>::new();

    for set in sets {
        merged
            .entry((
                set.required.keys().cloned().collect::<Vec<_>>(),
                set.optional.keys().cloned().collect::<Vec<_>>(),
            ))
            .and_modify(|merged| *merged = merged.merge(&set))
            .or_insert(set);
    }

    merged.into_values().collect()
}

fn name_choice(names: &BTreeMap<String, Value>) -> Vec<AttributeSet> {
    names
        .iter()
        .map(|(name, value)| AttributeSet {
            required: [(name.clone(), value.clone())].into(),
            optional: names
                .iter()
                .filter(|(other, _)| *other != name)
                .map(|(other, value)| (other.clone(), value.clone()))
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Literal;
    use pretty_assertions::assert_eq;

    fn attribute(name: &str) -> Pattern {
        Pattern::Attribute([name.into()].into(), Value::Any)
    }

    fn literals(values: &[&str]) -> Value {
        Value::Literals(
            values
                .iter()
                .map(|&value| Literal::Token(value.into()))
                .collect(),
        )
    }

    fn set(required: &[&str], optional: &[&str]) -> AttributeSet {
        AttributeSet {
            required: required
                .iter()
                .map(|&name| (name.into(), Value::Any))
                .collect(),
            optional: optional
                .iter()
                .map(|&name| (name.into(), Value::Any))
                .collect(),
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
            normalize_attributes(&Pattern::Attribute(
                ["foo".into(), "bar".into()].into(),
                Value::Any
            ))
            .unwrap(),
            vec![set(&["bar"], &["foo"]), set(&["foo"], &["bar"])]
        );
    }

    #[test]
    fn normalize_optional_alternative_attribute_names() {
        assert_eq!(
            normalize_attributes(&Pattern::optional(Pattern::Attribute(
                ["foo".into(), "bar".into()].into(),
                Value::Any
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
    fn merge_values_of_alternative_attributes() {
        assert_eq!(
            normalize_attributes(&Pattern::choice([
                Pattern::Attribute(["foo".into()].into(), literals(&["bar"])),
                Pattern::Attribute(["foo".into()].into(), literals(&["baz"])),
            ]))
            .unwrap(),
            vec![AttributeSet {
                required: [("foo".into(), literals(&["bar", "baz"]))].into(),
                optional: Default::default(),
            }]
        );
    }

    #[test]
    fn collapse_optional_choice_of_same_name_attributes() {
        assert_eq!(
            normalize_attributes(&Pattern::optional(Pattern::choice([
                Pattern::Attribute(["foo".into()].into(), literals(&["bar"])),
                Pattern::Attribute(["foo".into()].into(), literals(&["baz"])),
            ])))
            .unwrap(),
            vec![AttributeSet {
                required: Default::default(),
                optional: [("foo".into(), literals(&["bar", "baz"]))].into(),
            }]
        );
    }

    #[test]
    fn keep_values_of_exclusive_attributes() {
        // Values of attributes in distinct alternatives stay separate.
        assert_eq!(
            normalize_attributes(&Pattern::choice([
                Pattern::Attribute(["foo".into()].into(), literals(&["bar"])),
                Pattern::Attribute(["baz".into()].into(), literals(&["qux"])),
            ]))
            .unwrap(),
            vec![
                AttributeSet {
                    required: [("baz".into(), literals(&["qux"]))].into(),
                    optional: Default::default(),
                },
                AttributeSet {
                    required: [("foo".into(), literals(&["bar"]))].into(),
                    optional: Default::default(),
                },
            ]
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
