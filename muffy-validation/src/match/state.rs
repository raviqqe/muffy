use crate::content::Content;
use alloc::collections::BTreeSet;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum State {
    Choice(Vec<Self>),
    Content(&'static Content),
    Empty,
    Group(Vec<Self>),
    Interleave(Vec<Self>),
    NotAllowed,
}

impl State {
    pub fn choice(states: impl IntoIterator<Item = Self>) -> Self {
        let mut alternatives = BTreeSet::new();

        for state in states {
            match state {
                Self::Choice(states) => alternatives.extend(states),
                Self::NotAllowed => {}
                state => {
                    alternatives.insert(state);
                }
            }
        }

        if alternatives.is_empty() {
            Self::NotAllowed
        } else if alternatives.len() == 1
            && let Some(alternative) = alternatives.pop_first()
        {
            alternative
        } else {
            Self::Choice(alternatives.into_iter().collect())
        }
    }

    pub fn group(states: impl IntoIterator<Item = Self>) -> Self {
        let mut sequence = vec![];

        for state in states {
            match state {
                Self::Empty => {}
                Self::Group(states) => sequence.extend(states),
                Self::NotAllowed => return Self::NotAllowed,
                state => sequence.push(state),
            }
        }

        if sequence.is_empty() {
            Self::Empty
        } else if sequence.len() == 1
            && let Some(state) = sequence.pop()
        {
            state
        } else {
            Self::Group(sequence)
        }
    }

    pub fn interleave(states: impl IntoIterator<Item = Self>) -> Self {
        let mut operands = vec![];

        for state in states {
            match state {
                Self::Empty => {}
                Self::Interleave(states) => operands.extend(states),
                Self::NotAllowed => return Self::NotAllowed,
                state => operands.push(state),
            }
        }

        operands.sort();

        if operands.is_empty() {
            Self::Empty
        } else if operands.len() == 1
            && let Some(state) = operands.pop()
        {
            state
        } else {
            Self::Interleave(operands)
        }
    }

    pub fn nullable(&self) -> bool {
        match self {
            Self::Choice(states) => states.iter().any(Self::nullable),
            Self::Content(content) => content.nullable(),
            Self::Empty => true,
            Self::Group(states) | Self::Interleave(states) => states.iter().all(Self::nullable),
            Self::NotAllowed => false,
        }
    }
}
