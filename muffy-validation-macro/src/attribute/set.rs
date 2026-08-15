use crate::value::Value;
use alloc::collections::BTreeMap;

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct AttributeSet {
    pub required: BTreeMap<String, Value>,
    pub optional: BTreeMap<String, Value>,
}

impl AttributeSet {
    pub fn merge(&self, other: &Self) -> Self {
        Self {
            required: merge_attributes(&self.required, &other.required),
            optional: merge_attributes(&self.optional, &other.optional),
        }
    }
}

pub fn merge_attributes(
    attributes: &BTreeMap<String, Value>,
    others: &BTreeMap<String, Value>,
) -> BTreeMap<String, Value> {
    let mut merged = attributes.clone();

    for (name, value) in others {
        merged
            .entry(name.clone())
            .and_modify(|merged| *merged = merged.merge(value))
            .or_insert_with(|| value.clone());
    }

    merged
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Literal;
    use pretty_assertions::assert_eq;

    fn attributes(names: &[&str]) -> BTreeMap<String, Value> {
        names
            .iter()
            .map(|&name| (name.into(), Value::Any))
            .collect()
    }

    #[test]
    fn merge_sets() {
        assert_eq!(
            AttributeSet {
                required: attributes(&["foo"]),
                optional: attributes(&["bar"]),
            }
            .merge(&AttributeSet {
                required: attributes(&["baz"]),
                optional: attributes(&["bar", "qux"]),
            }),
            AttributeSet {
                required: attributes(&["baz", "foo"]),
                optional: attributes(&["bar", "qux"]),
            }
        );
    }

    #[test]
    fn merge_values_of_shared_attribute() {
        assert_eq!(
            AttributeSet {
                required: [(
                    "foo".into(),
                    Value::Literals([Literal::Token("bar".into())].into())
                )]
                .into(),
                optional: Default::default(),
            }
            .merge(&AttributeSet {
                required: [(
                    "foo".into(),
                    Value::Literals([Literal::Token("baz".into())].into())
                )]
                .into(),
                optional: Default::default(),
            }),
            AttributeSet {
                required: [(
                    "foo".into(),
                    Value::Literals(
                        [Literal::Token("bar".into()), Literal::Token("baz".into())].into()
                    )
                )]
                .into(),
                optional: Default::default(),
            }
        );
    }
}
