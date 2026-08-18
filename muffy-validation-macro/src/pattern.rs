mod normal;

pub use self::normal::normalize_pattern;
use crate::value::Value;
use alloc::collections::BTreeSet;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Pattern {
    Attribute(BTreeSet<String>, Value),
    Choice(Vec<Self>),
    Element(BTreeSet<String>),
    Empty,
    Group(Vec<Self>),
    Interleave(Vec<Self>),
    Many0(Box<Self>),
    Many1(Box<Self>),
    NotAllowed,
    Optional(Box<Self>),
    Text,
}

impl Pattern {
    pub fn choice(patterns: impl IntoIterator<Item = Self>) -> Self {
        let mut alternatives = BTreeSet::new();
        let mut names = BTreeSet::new();
        let mut empty = false;

        for pattern in patterns {
            match pattern {
                Self::Choice(patterns) => {
                    for pattern in patterns {
                        if let Self::Element(elements) = pattern {
                            names.extend(elements);
                        } else {
                            alternatives.insert(pattern);
                        }
                    }
                }
                Self::Element(elements) => names.extend(elements),
                Self::Empty => empty = true,
                Self::NotAllowed => {}
                pattern => {
                    alternatives.insert(pattern);
                }
            }
        }

        if !names.is_empty() {
            // Merge all element names into a single term.
            alternatives.insert(Self::Element(names));
        }

        let pattern = if alternatives.is_empty() {
            Self::NotAllowed
        } else if alternatives.len() == 1
            && let Some(alternative) = alternatives.pop_first()
        {
            alternative
        } else {
            Self::Choice(alternatives.into_iter().collect())
        };

        if empty {
            Self::optional(pattern)
        } else {
            pattern
        }
    }

    pub fn group(patterns: impl IntoIterator<Item = Self>) -> Self {
        let mut sequence = vec![];

        for pattern in patterns {
            match pattern {
                Self::Empty => {}
                Self::NotAllowed => return Self::NotAllowed,
                Self::Group(patterns) => sequence.extend(patterns),
                pattern => sequence.push(pattern),
            }
        }

        if sequence.is_empty() {
            Self::Empty
        } else if sequence.len() == 1
            && let Some(pattern) = sequence.pop()
        {
            pattern
        } else {
            Self::Group(sequence)
        }
    }

    pub fn interleave(patterns: impl IntoIterator<Item = Self>) -> Self {
        let mut operands = vec![];

        for pattern in patterns {
            match pattern {
                Self::Empty => {}
                Self::NotAllowed => return Self::NotAllowed,
                Self::Interleave(patterns) => operands.extend(patterns),
                pattern => operands.push(pattern),
            }
        }

        operands.sort();

        if operands.is_empty() {
            Self::Empty
        } else if operands.len() == 1
            && let Some(operand) = operands.pop()
        {
            operand
        } else {
            Self::Interleave(operands)
        }
    }

    pub fn many0(pattern: Self) -> Self {
        match pattern {
            Self::Empty | Self::NotAllowed => Self::Empty,
            Self::Many0(pattern) | Self::Many1(pattern) | Self::Optional(pattern) => {
                Self::Many0(pattern)
            }
            pattern => Self::Many0(pattern.into()),
        }
    }

    pub fn many1(pattern: Self) -> Self {
        match pattern {
            Self::Empty => Self::Empty,
            Self::NotAllowed => Self::NotAllowed,
            Self::Many0(pattern) | Self::Optional(pattern) => Self::Many0(pattern),
            Self::Many1(pattern) => Self::Many1(pattern),
            pattern => Self::Many1(pattern.into()),
        }
    }

    pub fn optional(pattern: Self) -> Self {
        match pattern {
            Self::Empty | Self::NotAllowed => Self::Empty,
            Self::Many0(pattern) | Self::Many1(pattern) => Self::Many0(pattern),
            Self::Optional(pattern) => Self::Optional(pattern),
            pattern => Self::Optional(pattern.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn element(name: &str) -> Pattern {
        Pattern::Element([name.into()].into())
    }

    #[test]
    fn merge_element_names() {
        assert_eq!(
            Pattern::choice([element("foo"), element("bar")]),
            Pattern::Element(["bar".into(), "foo".into()].into())
        );
    }

    #[test]
    fn merge_element_names_in_nested_choice() {
        assert_eq!(
            Pattern::choice([
                Pattern::choice([element("foo"), element("bar")]),
                element("baz")
            ]),
            Pattern::Element(["bar".into(), "baz".into(), "foo".into()].into())
        );
    }

    #[test]
    fn keep_other_alternatives_beside_elements() {
        assert_eq!(
            Pattern::choice([element("foo"), Pattern::Text]),
            Pattern::Choice(vec![Pattern::Element(["foo".into()].into()), Pattern::Text])
        );
    }

    #[test]
    fn merge_optional_element_names() {
        assert_eq!(
            Pattern::choice([element("foo"), element("bar"), Pattern::Empty]),
            Pattern::optional(Pattern::Element(["bar".into(), "foo".into()].into()))
        );
    }
}
