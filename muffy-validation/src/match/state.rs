use crate::content::{Content, TEXT_TOKEN};
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
    pub fn step(&self, name: &str) -> Self {
        match self {
            Self::Choice(states) => Self::choice(states.iter().map(|state| state.step(name))),
            Self::Content(content) => step_content(content, name),
            Self::Group(states) => {
                let mut alternatives = vec![];

                for (index, operand) in states.iter().enumerate() {
                    alternatives.push(Self::group(
                        [operand.step(name)]
                            .into_iter()
                            .chain(states[index + 1..].iter().cloned()),
                    ));

                    if !operand.is_nullable() {
                        break;
                    }
                }

                Self::choice(alternatives)
            }
            Self::Interleave(states) => Self::choice((0..states.len()).map(|index| {
                Self::interleave(states.iter().enumerate().map(|(other, operand)| {
                    if other == index {
                        operand.step(name)
                    } else {
                        operand.clone()
                    }
                }))
            })),
            Self::Empty | Self::NotAllowed => Self::NotAllowed,
        }
    }

    pub fn is_nullable(&self) -> bool {
        match self {
            Self::Choice(states) => states.iter().any(Self::is_nullable),
            Self::Content(content) => content.nullable(),
            Self::Empty => true,
            Self::Group(states) | Self::Interleave(states) => states.iter().all(Self::is_nullable),
            Self::NotAllowed => false,
        }
    }

    fn choice(states: impl IntoIterator<Item = Self>) -> Self {
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

    fn group(states: impl IntoIterator<Item = Self>) -> Self {
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

    fn interleave(states: impl IntoIterator<Item = Self>) -> Self {
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
}

fn step_content(content: &'static Content, name: &str) -> State {
    match content {
        Content::Choice(patterns) => {
            State::choice(patterns.iter().map(|pattern| step_content(pattern, name)))
        }
        Content::Element(names) => {
            if names.binary_search(&name).is_ok() {
                State::Empty
            } else {
                State::NotAllowed
            }
        }
        Content::Empty => State::NotAllowed,
        Content::Group(patterns) => {
            State::Group(patterns.iter().map(State::Content).collect()).step(name)
        }
        Content::Interleave(patterns) => {
            State::Interleave(patterns.iter().map(State::Content).collect()).step(name)
        }
        Content::Many0(operand) => {
            State::group([step_content(operand, name), State::Content(content)])
        }
        Content::Many1(operand) => State::group([
            step_content(operand, name),
            State::choice([State::Empty, State::Content(content)]),
        ]),
        Content::Optional(operand) => step_content(operand, name),
        Content::Text => {
            if name == TEXT_TOKEN {
                State::Content(content)
            } else {
                State::NotAllowed
            }
        }
    }
}
