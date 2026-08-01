use crate::error::{AttributeError, ChildError, MarkupError};
use alloc::collections::{BTreeMap, BTreeSet};
use muffy_document::html::{Element, Node};
use regex::Regex;

const TEXT_TOKEN: &str = "#text";

const EMPTY_SET: AttributeSet = AttributeSet {
    required: &[],
    optional: &[],
};

pub struct AttributeSet {
    pub required: &'static [&'static str],
    pub optional: &'static [&'static str],
}

/// A content pattern over child element names and text tokens.
#[derive(Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Content {
    Choice(&'static [Self]),
    /// Sorted alternative names of an element.
    Element(&'static [&'static str]),
    Empty,
    Group(&'static [Self]),
    Interleave(&'static [Self]),
    Many0(&'static Self),
    Many1(&'static Self),
    Optional(&'static Self),
    Text,
}

/// One variant definition of an element in a schema.
pub struct Variant {
    pub attributes: &'static [AttributeSet],
    pub content: &'static Content,
}

/// A validation rule of an element: unions of allowed names for coarse checks
/// and per-variant rules for co-occurrence and ordering checks.
pub struct Rule {
    pub attributes: &'static [&'static str],
    pub children: &'static [&'static str],
    pub variants: &'static [Variant],
}

/// A residual content pattern after matching a prefix of children.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum State {
    Choice(Vec<Self>),
    Content(&'static Content),
    Empty,
    Group(Vec<Self>),
    Interleave(Vec<Self>),
    NotAllowed,
}

impl State {
    fn choice(states: impl IntoIterator<Item = Self>) -> Self {
        let mut alternatives = BTreeSet::new();

        for state in states {
            match state {
                Self::NotAllowed => {}
                Self::Choice(states) => alternatives.extend(states),
                state => {
                    alternatives.insert(state);
                }
            }
        }

        if alternatives.len() == 1 {
            alternatives.pop_first().expect("alternative")
        } else if alternatives.is_empty() {
            Self::NotAllowed
        } else {
            Self::Choice(alternatives.into_iter().collect())
        }
    }

    fn group(states: impl IntoIterator<Item = Self>) -> Self {
        let mut sequence = vec![];

        for state in states {
            match state {
                Self::Empty => {}
                Self::NotAllowed => return Self::NotAllowed,
                Self::Group(states) => sequence.extend(states),
                state => sequence.push(state),
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

    fn interleave(states: impl IntoIterator<Item = Self>) -> Self {
        let mut operands = vec![];

        for state in states {
            match state {
                Self::Empty => {}
                Self::NotAllowed => return Self::NotAllowed,
                Self::Interleave(states) => operands.extend(states),
                state => operands.push(state),
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

    fn nullable(&self) -> bool {
        match self {
            Self::Choice(states) => states.iter().any(Self::nullable),
            Self::Content(content) => content.nullable(),
            Self::Empty => true,
            Self::Group(states) | Self::Interleave(states) => states.iter().all(Self::nullable),
            Self::NotAllowed => false,
        }
    }
}

impl Content {
    fn nullable(&self) -> bool {
        match self {
            Self::Choice(patterns) => patterns.iter().any(Self::nullable),
            Self::Element(_) => false,
            Self::Empty | Self::Many0(_) | Self::Optional(_) | Self::Text => true,
            Self::Group(patterns) | Self::Interleave(patterns) => {
                patterns.iter().all(Self::nullable)
            }
            Self::Many1(pattern) => pattern.nullable(),
        }
    }
}

fn step(state: &State, name: &str) -> State {
    match state {
        State::Choice(states) => State::choice(states.iter().map(|state| step(state, name))),
        State::Content(content) => step_content(content, name),
        State::Group(states) => {
            let mut alternatives = vec![];

            for (index, operand) in states.iter().enumerate() {
                alternatives.push(State::group(
                    [step(operand, name)]
                        .into_iter()
                        .chain(states[index + 1..].iter().cloned()),
                ));

                if !operand.nullable() {
                    break;
                }
            }

            State::choice(alternatives)
        }
        State::Interleave(states) => State::choice((0..states.len()).map(|index| {
            State::interleave(states.iter().enumerate().map(|(other, operand)| {
                if other == index {
                    step(operand, name)
                } else {
                    operand.clone()
                }
            }))
        })),
        State::Empty | State::NotAllowed => State::NotAllowed,
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
        Content::Group(patterns) => step(
            &State::Group(patterns.iter().map(State::Content).collect()),
            name,
        ),
        Content::Interleave(patterns) => step(
            &State::Interleave(patterns.iter().map(State::Content).collect()),
            name,
        ),
        Content::Many0(operand) => {
            State::group([step_content(operand, name), State::Content(content)])
        }
        // The rest of a repetition matched once is a zero-or-more repetition.
        Content::Many1(operand) => State::group([
            step_content(operand, name),
            State::choice([State::Empty, State::Content(content)]),
        ]),
        Content::Optional(operand) => step_content(operand, name),
        // A text pattern matches any number of text nodes.
        Content::Text => {
            if name == TEXT_TOKEN {
                State::Content(content)
            } else {
                State::NotAllowed
            }
        }
    }
}

fn match_children(
    content: &'static Content,
    children: &[(&'static str, bool)],
) -> (BTreeSet<&'static str>, State) {
    let mut state = State::Content(content);
    let mut misplaced = BTreeSet::new();

    for (name, exempt) in children {
        let next = step(&state, name);

        if next == State::NotAllowed {
            if !exempt {
                misplaced.insert(*name);
            }
        } else {
            state = next;
        }
    }

    (misplaced, state)
}

// Names on shortest paths from a state to acceptance.
fn expected_names(state: &State, names: &[&'static str]) -> BTreeSet<&'static str> {
    if state.nullable() {
        return Default::default();
    }

    let mut visited = BTreeSet::from([state.clone()]);
    let mut frontier = vec![(state.clone(), None)];

    loop {
        let mut expected = BTreeSet::new();
        let mut next_frontier = vec![];

        for (state, first) in &frontier {
            for name in names {
                let next = step(state, name);

                if next == State::NotAllowed {
                    continue;
                }

                let first = first.unwrap_or(name);

                if next.nullable() {
                    expected.insert(*first);
                }

                if visited.insert(next.clone()) {
                    next_frontier.push((next, Some(first)));
                }
            }
        }

        if !expected.is_empty() || next_frontier.is_empty() {
            return expected;
        }

        frontier = next_frontier;
    }
}

pub fn validate_rule(
    element: &Element,
    ignored_attributes: &[Regex],
    ignored_elements: &[Regex],
    rule: &Rule,
) -> Result<(), MarkupError> {
    let mut attribute_errors = BTreeMap::<String, BTreeSet<AttributeError>>::new();
    let mut child_errors = BTreeMap::<String, BTreeSet<ChildError>>::new();

    let mut attributes = vec![];
    // Ignored names satisfy schema requirements but never produce errors.
    let mut exempt_attributes = vec![];

    for (name, _) in element.attributes() {
        let ignored = ignored_attributes
            .iter()
            .any(|pattern| pattern.is_match(name));

        if let Ok(index) = rule.attributes.binary_search(&name) {
            if ignored {
                exempt_attributes.push(rule.attributes[index]);
            } else {
                attributes.push(rule.attributes[index]);
            }
        } else if !ignored {
            attribute_errors
                .entry(name.into())
                .or_default()
                .insert(AttributeError::NotAllowed);
        }
    }

    attributes.sort_unstable();
    exempt_attributes.sort_unstable();

    let mut children = vec![];

    for child in element.children() {
        let name = match child {
            Node::Element(child) => child.name(),
            Node::Text(text) => {
                if text.chars().all(char::is_whitespace) {
                    continue;
                }

                TEXT_TOKEN
            }
        };
        let ignored = ignored_elements
            .iter()
            .any(|pattern| pattern.is_match(name));

        if let Ok(index) = rule.children.binary_search(&name) {
            children.push((rule.children[index], ignored));
        } else if !ignored {
            child_errors
                .entry(name.into())
                .or_default()
                .insert(ChildError::NotAllowed);
        }
    }

    let (missing_attributes, missing_children) = if let Some(variant) = rule
        .variants
        .iter()
        .min_by_key(|variant| score_variant(variant, &attributes, &exempt_attributes, &children))
    {
        let set = variant
            .attributes
            .iter()
            .min_by_key(|set| set_score(set, &attributes, &exempt_attributes))
            .unwrap_or(&EMPTY_SET);

        for name in attributes.iter().filter(|name| {
            set.required.binary_search(name).is_err() && set.optional.binary_search(name).is_err()
        }) {
            attribute_errors
                .entry((*name).into())
                .or_default()
                .insert(AttributeError::Conflict);
        }

        let (misplaced, state) = match_children(variant.content, &children);

        for name in &misplaced {
            child_errors
                .entry((*name).into())
                .or_default()
                .insert(ChildError::Misplaced);
        }

        (
            set.required
                .iter()
                .filter(|name| {
                    attributes.binary_search(name).is_err()
                        && exempt_attributes.binary_search(name).is_err()
                        && !ignored_attributes
                            .iter()
                            .any(|pattern| pattern.is_match(name))
                })
                .map(|&name| name.into())
                .collect::<BTreeSet<String>>(),
            if misplaced.is_empty() && !state.nullable() {
                expected_names(&state, rule.children)
                    .iter()
                    .filter(|name| {
                        !ignored_elements
                            .iter()
                            .any(|pattern| pattern.is_match(name))
                    })
                    .map(|&name| name.into())
                    .collect::<BTreeSet<String>>()
            } else {
                Default::default()
            },
        )
    } else {
        Default::default()
    };

    if attribute_errors.is_empty()
        && child_errors.is_empty()
        && missing_attributes.is_empty()
        && missing_children.is_empty()
    {
        Ok(())
    } else {
        Err(MarkupError::InvalidElement {
            invalid_attributes: attribute_errors,
            invalid_children: child_errors,
            missing_attributes,
            missing_children,
        })
    }
}

// Variants are scored by the total error count, then by errors on present
// names so that a variant missing a name is preferred over one conflicting
// with an equal number of present names, and then by the requirement count so
// that the simplest variant is reported.
fn score_variant(
    variant: &Variant,
    attributes: &[&'static str],
    exempt_attributes: &[&'static str],
    children: &[(&'static str, bool)],
) -> (usize, usize, usize) {
    let (attribute_error_count, requirement_count, conflict_count) = variant
        .attributes
        .iter()
        .map(|set| set_score(set, attributes, exempt_attributes))
        .min()
        .unwrap_or_default();

    let (misplaced, state) = match_children(variant.content, children);

    (
        attribute_error_count
            + misplaced.len()
            + usize::from(misplaced.is_empty() && !state.nullable()),
        conflict_count + misplaced.len(),
        requirement_count,
    )
}

// Sets are ordered by the error count first and the requirement count second
// so that the simplest of equally scored alternatives is diagnosed.
fn set_score(
    set: &AttributeSet,
    attributes: &[&'static str],
    exempt_attributes: &[&'static str],
) -> (usize, usize, usize) {
    let conflict_count = attributes
        .iter()
        .filter(|name| {
            set.required.binary_search(name).is_err() && set.optional.binary_search(name).is_err()
        })
        .count();
    let missing_count = set
        .required
        .iter()
        .filter(|name| {
            attributes.binary_search(name).is_err()
                && exempt_attributes.binary_search(name).is_err()
        })
        .count();

    (
        conflict_count + missing_count,
        set.required.len(),
        conflict_count,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::sync::Arc;

    static EMPTY_CONTENT: Content = Content::Empty;

    // The content model of `element example { (attribute foo { text },
    // attribute bar { text }?) | attribute baz { text } }` with children
    // `(one, two?)`.
    static RULE: Rule = Rule {
        attributes: &["bar", "baz", "foo"],
        children: &["one", "two"],
        variants: &[
            Variant {
                attributes: &[AttributeSet {
                    required: &["foo"],
                    optional: &["bar"],
                }],
                content: &Content::Group(&[
                    Content::Element(&["one"]),
                    Content::Optional(&Content::Element(&["two"])),
                ]),
            },
            Variant {
                attributes: &[AttributeSet {
                    required: &["baz"],
                    optional: &[],
                }],
                content: &EMPTY_CONTENT,
            },
        ],
    };

    fn create_element(name: &str, attributes: Vec<(&str, &str)>, children: Vec<&str>) -> Element {
        Element::new(
            name.into(),
            attributes
                .into_iter()
                .map(|(name, value)| (name.into(), value.into()))
                .collect(),
            children
                .into_iter()
                .map(|name| Arc::new(Node::Element(Element::new(name.into(), vec![], vec![]))))
                .collect(),
        )
    }

    fn accepts(content: &'static Content, names: &'static [&'static str]) -> bool {
        let (misplaced, state) = match_children(
            content,
            &names.iter().map(|&name| (name, false)).collect::<Vec<_>>(),
        );

        misplaced.is_empty() && state.nullable()
    }

    mod content {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn accept_ordered_group() {
            static CONTENT: Content =
                Content::Group(&[Content::Element(&["foo"]), Content::Element(&["bar"])]);

            assert!(accepts(&CONTENT, &["foo", "bar"]));
            assert!(!accepts(&CONTENT, &["bar", "foo"]));
            assert!(!accepts(&CONTENT, &["foo"]));
            assert!(!accepts(&CONTENT, &["foo", "bar", "foo"]));
        }

        #[test]
        fn accept_optional_operand() {
            static CONTENT: Content = Content::Group(&[
                Content::Optional(&Content::Element(&["foo"])),
                Content::Element(&["bar"]),
            ]);

            assert!(accepts(&CONTENT, &["bar"]));
            assert!(accepts(&CONTENT, &["foo", "bar"]));
            assert!(!accepts(&CONTENT, &["foo"]));
        }

        #[test]
        fn accept_interleave_in_any_order() {
            static CONTENT: Content =
                Content::Interleave(&[Content::Element(&["foo"]), Content::Element(&["bar"])]);

            assert!(accepts(&CONTENT, &["foo", "bar"]));
            assert!(accepts(&CONTENT, &["bar", "foo"]));
            assert!(!accepts(&CONTENT, &["foo"]));
        }

        #[test]
        fn accept_repetition() {
            static CONTENT: Content = Content::Many0(&Content::Element(&["foo"]));

            assert!(accepts(&CONTENT, &[]));
            assert!(accepts(&CONTENT, &["foo", "foo", "foo"]));
            assert!(!accepts(&CONTENT, &["bar"]));
        }

        #[test]
        fn accept_at_least_one_repetition() {
            static CONTENT: Content = Content::Many1(&Content::Element(&["foo"]));

            assert!(!accepts(&CONTENT, &[]));
            assert!(accepts(&CONTENT, &["foo"]));
            assert!(accepts(&CONTENT, &["foo", "foo"]));
        }

        #[test]
        fn accept_repetition_interleaved_with_group() {
            static CONTENT: Content = Content::Interleave(&[
                Content::Group(&[Content::Element(&["foo"]), Content::Element(&["bar"])]),
                Content::Many0(&Content::Element(&["baz"])),
            ]);

            assert!(accepts(&CONTENT, &["baz", "foo", "baz", "bar", "baz"]));
            assert!(accepts(&CONTENT, &["foo", "bar"]));
            assert!(!accepts(&CONTENT, &["bar", "baz", "foo"]));
        }

        #[test]
        fn accept_text() {
            static CONTENT: Content = Content::Many0(&Content::Choice(&[
                Content::Text,
                Content::Element(&["foo"]),
            ]));

            assert!(accepts(&CONTENT, &[]));
            assert!(accepts(&CONTENT, &["foo", "foo"]));
            assert!(accepts(&CONTENT, &["#text", "foo", "#text"]));
        }

        #[test]
        fn expect_names_on_shortest_accepting_path() {
            static CONTENT: Content = Content::Interleave(&[
                Content::Group(&[Content::Element(&["foo"]), Content::Element(&["bar"])]),
                Content::Many0(&Content::Element(&["baz"])),
            ]);
            let names = ["bar", "baz", "foo"];

            let state = State::Content(&CONTENT);

            assert_eq!(expected_names(&state, &names), ["foo"].into());

            let state = step(&state, "foo");

            assert_eq!(expected_names(&state, &names), ["bar"].into());

            let state = step(&state, "bar");

            assert!(state.nullable());
            assert_eq!(expected_names(&state, &names), [].into());
        }
    }

    use pretty_assertions::assert_eq;

    #[test]
    fn validate_first_variant() {
        assert_eq!(
            validate_rule(
                &create_element("example", vec![("foo", ""), ("bar", "")], vec!["one"]),
                &[],
                &[],
                &RULE,
            ),
            Ok(())
        );
    }

    #[test]
    fn validate_second_variant() {
        assert_eq!(
            validate_rule(
                &create_element("example", vec![("baz", "")], vec![]),
                &[],
                &[],
                &RULE,
            ),
            Ok(())
        );
    }

    #[test]
    fn validate_unknown_attribute() {
        assert_eq!(
            validate_rule(
                &create_element("example", vec![("foo", ""), ("unknown", "")], vec!["one"]),
                &[],
                &[],
                &RULE,
            ),
            Err(MarkupError::InvalidElement {
                invalid_attributes: [("unknown".into(), [AttributeError::NotAllowed].into())]
                    .into(),
                invalid_children: Default::default(),
                missing_attributes: Default::default(),
                missing_children: Default::default(),
            })
        );
    }

    #[test]
    fn validate_conflicting_attributes() {
        assert_eq!(
            validate_rule(
                &create_element("example", vec![("baz", ""), ("bar", "")], vec![]),
                &[],
                &[],
                &RULE,
            ),
            Err(MarkupError::InvalidElement {
                invalid_attributes: [("bar".into(), [AttributeError::Conflict].into())].into(),
                invalid_children: Default::default(),
                missing_attributes: Default::default(),
                missing_children: Default::default(),
            })
        );
    }

    #[test]
    fn validate_missing_attribute() {
        assert_eq!(
            validate_rule(
                &create_element("example", vec![("bar", "")], vec!["one"]),
                &[],
                &[],
                &RULE,
            ),
            Err(MarkupError::InvalidElement {
                invalid_attributes: Default::default(),
                invalid_children: Default::default(),
                missing_attributes: ["foo".into()].into(),
                missing_children: Default::default(),
            })
        );
    }

    #[test]
    fn validate_misplaced_child() {
        assert_eq!(
            validate_rule(
                &create_element("example", vec![("foo", "")], vec!["two", "one"]),
                &[],
                &[],
                &RULE,
            ),
            Err(MarkupError::InvalidElement {
                invalid_attributes: Default::default(),
                invalid_children: [("two".into(), [ChildError::Misplaced].into())].into(),
                missing_attributes: Default::default(),
                missing_children: Default::default(),
            })
        );
    }

    #[test]
    fn validate_missing_child() {
        assert_eq!(
            validate_rule(
                &create_element("example", vec![("foo", "")], vec![]),
                &[],
                &[],
                &RULE,
            ),
            Err(MarkupError::InvalidElement {
                invalid_attributes: Default::default(),
                invalid_children: Default::default(),
                missing_attributes: Default::default(),
                missing_children: ["one".into()].into(),
            })
        );
    }

    #[test]
    fn skip_ignored_attribute() {
        assert_eq!(
            validate_rule(
                &create_element("example", vec![("foo", ""), ("data-x", "")], vec!["one"]),
                &[Regex::new("^data-.*$").unwrap()],
                &[],
                &RULE,
            ),
            Ok(())
        );
    }

    #[test]
    fn skip_ignored_child() {
        assert_eq!(
            validate_rule(
                &create_element("example", vec![("foo", "")], vec!["one", "custom-x"]),
                &[],
                &[Regex::new("^custom-.*$").unwrap()],
                &RULE,
            ),
            Ok(())
        );
    }

    #[test]
    fn satisfy_requirement_with_ignored_attribute() {
        assert_eq!(
            validate_rule(
                &create_element("example", vec![("foo", "")], vec!["one"]),
                &[Regex::new("^foo$").unwrap()],
                &[],
                &RULE,
            ),
            Ok(())
        );
    }

    #[test]
    fn satisfy_requirement_with_ignored_child() {
        assert_eq!(
            validate_rule(
                &create_element("example", vec![("foo", "")], vec!["one"]),
                &[],
                &[Regex::new("^one$").unwrap()],
                &RULE,
            ),
            Ok(())
        );
    }

    #[test]
    fn suppress_missing_ignored_attribute() {
        assert_eq!(
            validate_rule(
                &create_element("example", vec![], vec!["one"]),
                &[Regex::new("^foo$").unwrap()],
                &[],
                &RULE,
            ),
            Ok(())
        );
    }

    #[test]
    fn suppress_missing_ignored_child() {
        assert_eq!(
            validate_rule(
                &create_element("example", vec![("foo", "")], vec![]),
                &[],
                &[Regex::new("^one$").unwrap()],
                &RULE,
            ),
            Ok(())
        );
    }

    #[test]
    fn skip_misplaced_ignored_child() {
        assert_eq!(
            validate_rule(
                &create_element("example", vec![("foo", "")], vec!["two", "one", "two"]),
                &[],
                &[Regex::new("^two$").unwrap()],
                &RULE,
            ),
            Ok(())
        );
    }

    #[test]
    fn prefer_variant_with_fewer_conflicts() {
        // The second variant conflicts with the present `bar` attribute while
        // the first one misses the required `foo` attribute.
        assert_eq!(
            validate_rule(
                &create_element("example", vec![("bar", "")], vec![]),
                &[],
                &[],
                &RULE,
            ),
            Err(MarkupError::InvalidElement {
                invalid_attributes: Default::default(),
                invalid_children: Default::default(),
                missing_attributes: ["foo".into()].into(),
                missing_children: ["one".into()].into(),
            })
        );
    }
}
