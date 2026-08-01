use alloc::collections::BTreeSet;

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct AttributeTerm {
    pub required: BTreeSet<String>,
    pub optional: BTreeSet<String>,
}

impl AttributeTerm {
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
    fn merge_terms() {
        assert_eq!(
            AttributeTerm {
                required: ["foo".into()].into(),
                optional: ["bar".into()].into(),
            }
            .merge(&AttributeTerm {
                required: ["baz".into()].into(),
                optional: ["bar".into(), "qux".into()].into(),
            }),
            AttributeTerm {
                required: ["baz".into(), "foo".into()].into(),
                optional: ["bar".into(), "qux".into()].into(),
            }
        );
    }
}
