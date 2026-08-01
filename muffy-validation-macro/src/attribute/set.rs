use alloc::collections::BTreeSet;

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct AttributeSet {
    pub required: BTreeSet<String>,
    pub optional: BTreeSet<String>,
}

impl AttributeSet {
    pub fn merge(&self, other: &Self) -> Self {
        Self {
            required: self.required.union(&other.required).cloned().collect(),
            optional: self.optional.union(&other.optional).cloned().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn merge_sets() {
        assert_eq!(
            AttributeSet {
                required: ["foo".into()].into(),
                optional: ["bar".into()].into(),
            }
            .merge(&AttributeSet {
                required: ["baz".into()].into(),
                optional: ["bar".into(), "qux".into()].into(),
            }),
            AttributeSet {
                required: ["baz".into(), "foo".into()].into(),
                optional: ["bar".into(), "qux".into()].into(),
            }
        );
    }
}
