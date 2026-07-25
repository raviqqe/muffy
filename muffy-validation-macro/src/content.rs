use crate::{error::MacroError, pattern::CompiledPattern};
use alloc::collections::{BTreeMap, BTreeSet};

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
