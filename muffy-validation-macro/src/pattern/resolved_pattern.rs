use alloc::collections::BTreeSet;

// TODO Support attribute value schemas.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ResolvedPattern {
    Attribute(BTreeSet<String>),
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

impl ResolvedPattern {
    pub fn choice(patterns: impl IntoIterator<Item = Self>) -> Self {
        let mut alternatives = BTreeSet::new();
        let mut nullable = false;

        for pattern in patterns {
            match pattern {
                Self::NotAllowed => {}
                Self::Empty => nullable = true,
                Self::Choice(patterns) => alternatives.extend(patterns),
                pattern => {
                    alternatives.insert(pattern);
                }
            }
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

        if nullable {
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

        if sequence.len() == 1 {
            sequence.pop().expect("operand")
        } else if sequence.is_empty() {
            Self::Empty
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

        if operands.len() == 1 {
            operands.pop().expect("operand")
        } else if operands.is_empty() {
            Self::Empty
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
