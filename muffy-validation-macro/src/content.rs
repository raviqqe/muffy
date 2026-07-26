use crate::{error::MacroError, pattern::CompiledPattern};
use alloc::collections::{BTreeMap, BTreeSet};

// cspell: ignore brzozowski

const STATE_LIMIT: usize = 512;

/// A deterministic automaton over child element names built from Brzozowski
/// derivatives of a content pattern.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ContentAutomaton {
    /// Transitions by child element name for each state.
    pub transitions: Vec<BTreeMap<String, usize>>,
    /// Whether each state accepts the end of content.
    pub accepting: Vec<bool>,
    /// Child element names on shortest paths from each state to acceptance.
    pub expected: Vec<BTreeSet<String>>,
}

pub fn compile_content_automaton(
    pattern: &CompiledPattern,
) -> Result<ContentAutomaton, MacroError> {
    let initial = erase_text(pattern)?;
    let mut states = vec![initial.clone()];
    let mut indexes = BTreeMap::from([(initial, 0)]);
    let mut transitions = vec![];
    let mut index = 0;

    while index < states.len() {
        let state = states[index].clone();
        let mut state_transitions = BTreeMap::new();

        for name in element_names(&state) {
            let next = derive(&state, &name);

            if next == CompiledPattern::NotAllowed {
                continue;
            }

            let next_index = *indexes.entry(next.clone()).or_insert_with(|| {
                states.push(next);
                states.len() - 1
            });

            state_transitions.insert(name, next_index);
        }

        if states.len() > STATE_LIMIT {
            return Err(MacroError::PatternLimit("content model states"));
        }

        transitions.push(state_transitions);
        index += 1;
    }

    let accepting = states
        .iter()
        .map(CompiledPattern::nullable)
        .collect::<Vec<_>>();

    Ok(ContentAutomaton {
        expected: expected_names(&transitions, &accepting),
        transitions,
        accepting,
    })
}

fn erase_text(pattern: &CompiledPattern) -> Result<CompiledPattern, MacroError> {
    Ok(match pattern {
        CompiledPattern::Text | CompiledPattern::Empty => CompiledPattern::Empty,
        CompiledPattern::Element(_) | CompiledPattern::NotAllowed => pattern.clone(),
        CompiledPattern::Attribute(_) => {
            return Err(MacroError::RncPattern("attribute in content pattern"));
        }
        CompiledPattern::Choice(patterns) => CompiledPattern::choice(
            patterns
                .iter()
                .map(erase_text)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        CompiledPattern::Group(patterns) => CompiledPattern::group(
            patterns
                .iter()
                .map(erase_text)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        CompiledPattern::Interleave(patterns) => CompiledPattern::interleave(
            patterns
                .iter()
                .map(erase_text)
                .collect::<Result<Vec<_>, _>>()?,
        ),
        CompiledPattern::Many0(pattern) => CompiledPattern::many0(erase_text(pattern)?),
        CompiledPattern::Many1(pattern) => CompiledPattern::many1(erase_text(pattern)?),
        CompiledPattern::Optional(pattern) => CompiledPattern::optional(erase_text(pattern)?),
    })
}

fn element_names(pattern: &CompiledPattern) -> BTreeSet<String> {
    match pattern {
        CompiledPattern::Element(names) => names.clone(),
        CompiledPattern::Choice(patterns)
        | CompiledPattern::Group(patterns)
        | CompiledPattern::Interleave(patterns) => {
            patterns.iter().flat_map(element_names).collect()
        }
        CompiledPattern::Many0(pattern)
        | CompiledPattern::Many1(pattern)
        | CompiledPattern::Optional(pattern) => element_names(pattern),
        CompiledPattern::Attribute(_)
        | CompiledPattern::Empty
        | CompiledPattern::NotAllowed
        | CompiledPattern::Text => Default::default(),
    }
}

fn derive(pattern: &CompiledPattern, name: &str) -> CompiledPattern {
    match pattern {
        CompiledPattern::Element(names) => {
            if names.contains(name) {
                CompiledPattern::Empty
            } else {
                CompiledPattern::NotAllowed
            }
        }
        CompiledPattern::Choice(patterns) => {
            CompiledPattern::choice(patterns.iter().map(|pattern| derive(pattern, name)))
        }
        CompiledPattern::Group(patterns) => {
            let mut alternatives = vec![];

            for (index, operand) in patterns.iter().enumerate() {
                alternatives.push(CompiledPattern::group(
                    [derive(operand, name)]
                        .into_iter()
                        .chain(patterns[index + 1..].iter().cloned()),
                ));

                if !operand.nullable() {
                    break;
                }
            }

            CompiledPattern::choice(alternatives)
        }
        CompiledPattern::Interleave(patterns) => {
            CompiledPattern::choice(patterns.iter().enumerate().map(|(index, operand)| {
                CompiledPattern::interleave(patterns.iter().enumerate().map(
                    |(other_index, other)| {
                        if other_index == index {
                            derive(operand, name)
                        } else {
                            other.clone()
                        }
                    },
                ))
            }))
        }
        CompiledPattern::Many0(operand) | CompiledPattern::Many1(operand) => {
            CompiledPattern::group([
                derive(operand, name),
                CompiledPattern::many0(operand.as_ref().clone()),
            ])
        }
        CompiledPattern::Optional(operand) => derive(operand, name),
        CompiledPattern::Attribute(_)
        | CompiledPattern::Empty
        | CompiledPattern::NotAllowed
        | CompiledPattern::Text => CompiledPattern::NotAllowed,
    }
}

fn expected_names(
    transitions: &[BTreeMap<String, usize>],
    accepting: &[bool],
) -> Vec<BTreeSet<String>> {
    let mut distances = accepting
        .iter()
        .map(|&accepting| if accepting { Some(0usize) } else { None })
        .collect::<Vec<_>>();

    loop {
        let mut changed = false;

        for (state, state_transitions) in transitions.iter().enumerate() {
            let distance = state_transitions
                .values()
                .filter_map(|&next| distances[next])
                .min()
                .map(|distance| distance + 1);

            if let Some(distance) = distance
                && distances[state].is_none_or(|current| distance < current)
            {
                distances[state] = Some(distance);
                changed = true;
            }
        }

        if !changed {
            break;
        }
    }

    transitions
        .iter()
        .enumerate()
        .map(|(state, state_transitions)| {
            if accepting[state] {
                Default::default()
            } else {
                state_transitions
                    .iter()
                    .filter(|(_, next)| {
                        distances[state].zip(distances[**next]).is_some_and(
                            |(state_distance, next_distance)| next_distance + 1 == state_distance,
                        )
                    })
                    .map(|(name, _)| name.clone())
                    .collect()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn element(name: &str) -> CompiledPattern {
        CompiledPattern::Element([name.into()].into())
    }

    fn accepts(pattern: &CompiledPattern, names: &[&str]) -> bool {
        let automaton = compile_content_automaton(pattern).unwrap();
        let mut state = 0;

        for name in names {
            let Some(&next) = automaton.transitions[state].get(*name) else {
                return false;
            };

            state = next;
        }

        automaton.accepting[state]
    }

    #[test]
    fn accept_ordered_group() {
        let pattern = CompiledPattern::group([element("foo"), element("bar")]);

        assert!(accepts(&pattern, &["foo", "bar"]));
        assert!(!accepts(&pattern, &["bar", "foo"]));
        assert!(!accepts(&pattern, &["foo"]));
        assert!(!accepts(&pattern, &["foo", "bar", "foo"]));
    }

    #[test]
    fn accept_optional_operand() {
        let pattern =
            CompiledPattern::group([CompiledPattern::optional(element("foo")), element("bar")]);

        assert!(accepts(&pattern, &["bar"]));
        assert!(accepts(&pattern, &["foo", "bar"]));
        assert!(!accepts(&pattern, &["foo"]));
    }

    #[test]
    fn accept_interleave_in_any_order() {
        let pattern = CompiledPattern::interleave([element("foo"), element("bar")]);

        assert!(accepts(&pattern, &["foo", "bar"]));
        assert!(accepts(&pattern, &["bar", "foo"]));
        assert!(!accepts(&pattern, &["foo"]));
    }

    #[test]
    fn accept_repetition() {
        let pattern = CompiledPattern::many0(element("foo"));

        assert!(accepts(&pattern, &[]));
        assert!(accepts(&pattern, &["foo", "foo", "foo"]));
        assert!(!accepts(&pattern, &["bar"]));
    }

    #[test]
    fn accept_repetition_interleaved_with_group() {
        let pattern = CompiledPattern::interleave([
            CompiledPattern::group([element("foo"), element("bar")]),
            CompiledPattern::many0(element("baz")),
        ]);

        assert!(accepts(&pattern, &["baz", "foo", "baz", "bar", "baz"]));
        assert!(accepts(&pattern, &["foo", "bar"]));
        assert!(!accepts(&pattern, &["bar", "baz", "foo"]));
    }

    #[test]
    fn erase_text() {
        let pattern = CompiledPattern::many0(CompiledPattern::choice([
            CompiledPattern::Text,
            element("foo"),
        ]));

        assert!(accepts(&pattern, &[]));
        assert!(accepts(&pattern, &["foo", "foo"]));
    }

    #[test]
    fn expect_names_on_shortest_accepting_path() {
        let automaton = compile_content_automaton(&CompiledPattern::interleave([
            CompiledPattern::group([element("foo"), element("bar")]),
            CompiledPattern::many0(element("baz")),
        ]))
        .unwrap();

        assert_eq!(automaton.expected[0], ["foo".into()].into());

        let state = automaton.transitions[0]["foo"];

        assert_eq!(automaton.expected[state], ["bar".into()].into());

        let state = automaton.transitions[state]["bar"];

        assert!(automaton.accepting[state]);
        assert_eq!(automaton.expected[state], [].into());
    }

    #[test]
    fn fail_on_attribute() {
        assert!(matches!(
            compile_content_automaton(&CompiledPattern::Attribute(["foo".into()].into())),
            Err(MacroError::RncPattern(_))
        ));
    }
}
