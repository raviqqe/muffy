use crate::error::{AttributeError, ChildError, MarkupError};
use alloc::collections::{BTreeMap, BTreeSet};
use muffy_document::html::{Element, Node};
use regex::Regex;

/// A pseudo-name of text nodes in child sequences.
const TEXT_TOKEN: &str = "#text";

const EMPTY_TERM: AttributeTerm = AttributeTerm {
    required: &[],
    optional: &[],
};

/// One alternative of attribute names an element accepts: all required names
/// must be present, and no name outside the required and optional ones may be.
pub struct AttributeTerm {
    pub required: &'static [&'static str],
    pub optional: &'static [&'static str],
}

/// A deterministic automaton over child element names and text tokens.
pub struct ContentAutomaton {
    /// Transitions sorted by child name for each state.
    pub transitions: &'static [&'static [(&'static str, usize)]],
    /// Whether each state accepts the end of content.
    pub accepting: &'static [bool],
    /// Child names on shortest paths from each state to acceptance.
    pub expected: &'static [&'static [&'static str]],
}

/// One variant definition of an element in a schema.
pub struct Variant {
    pub attributes: &'static [AttributeTerm],
    pub content: &'static ContentAutomaton,
}

/// Validation rules of an element: unions of allowed names for coarse checks
/// and per-variant rules for co-occurrence and ordering checks.
pub struct Rules {
    pub attributes: &'static [&'static str],
    pub children: &'static [&'static str],
    pub variants: &'static [Variant],
}

pub fn validate_rules(
    element: &Element,
    ignored_attributes: &[Regex],
    ignored_elements: &[Regex],
    rules: &Rules,
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

        if let Ok(index) = rules.attributes.binary_search(&name) {
            if ignored {
                exempt_attributes.push(rules.attributes[index]);
            } else {
                attributes.push(rules.attributes[index]);
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

        if let Ok(index) = rules.children.binary_search(&name) {
            children.push((rules.children[index], ignored));
        } else if !ignored {
            child_errors
                .entry(name.into())
                .or_default()
                .insert(ChildError::NotAllowed);
        }
    }

    let (missing_attributes, missing_children) = if let Some(variant) = rules
        .variants
        .iter()
        .min_by_key(|variant| score_variant(variant, &attributes, &exempt_attributes, &children))
    {
        let term = variant
            .attributes
            .iter()
            .min_by_key(|term| term_score(term, &attributes, &exempt_attributes))
            .unwrap_or(&EMPTY_TERM);

        for name in attributes.iter().filter(|name| {
            term.required.binary_search(name).is_err() && term.optional.binary_search(name).is_err()
        }) {
            attribute_errors
                .entry((*name).into())
                .or_default()
                .insert(AttributeError::Conflict);
        }

        let mut state = 0;
        let mut misplaced = BTreeSet::new();

        for (name, exempt) in &children {
            if let Ok(index) =
                variant.content.transitions[state].binary_search_by_key(name, |(name, _)| name)
            {
                state = variant.content.transitions[state][index].1;
            } else if !exempt {
                misplaced.insert(*name);
            }
        }

        for name in &misplaced {
            child_errors
                .entry((*name).into())
                .or_default()
                .insert(ChildError::Misplaced);
        }

        (
            term.required
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
            if misplaced.is_empty() && !variant.content.accepting[state] {
                variant.content.expected[state]
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
        .map(|term| term_score(term, attributes, exempt_attributes))
        .min()
        .unwrap_or_default();

    let mut state = 0;
    let mut misplaced_count = 0;

    for (name, exempt) in children {
        if let Ok(index) =
            variant.content.transitions[state].binary_search_by_key(name, |(name, _)| name)
        {
            state = variant.content.transitions[state][index].1;
        } else if !exempt {
            misplaced_count += 1;
        }
    }

    (
        attribute_error_count
            + misplaced_count
            + usize::from(misplaced_count == 0 && !variant.content.accepting[state]),
        conflict_count + misplaced_count,
        requirement_count,
    )
}

// Terms are ordered by the error count first and the requirement count second
// so that the simplest of equally scored alternatives is diagnosed.
fn term_score(
    term: &AttributeTerm,
    attributes: &[&'static str],
    exempt_attributes: &[&'static str],
) -> (usize, usize, usize) {
    let conflict_count = attributes
        .iter()
        .filter(|name| {
            term.required.binary_search(name).is_err() && term.optional.binary_search(name).is_err()
        })
        .count();
    let missing_count = term
        .required
        .iter()
        .filter(|name| {
            attributes.binary_search(name).is_err()
                && exempt_attributes.binary_search(name).is_err()
        })
        .count();

    (
        conflict_count + missing_count,
        term.required.len(),
        conflict_count,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::sync::Arc;
    use pretty_assertions::assert_eq;

    static EMPTY_CONTENT: ContentAutomaton = ContentAutomaton {
        transitions: &[&[]],
        accepting: &[true],
        expected: &[&[]],
    };

    // The content model of `element example { (attribute foo { text },
    // attribute bar { text }?) | attribute baz { text } }` with children
    // `(one, two?)`.
    static RULES: Rules = Rules {
        attributes: &["bar", "baz", "foo"],
        children: &["one", "two"],
        variants: &[
            Variant {
                attributes: &[AttributeTerm {
                    required: &["foo"],
                    optional: &["bar"],
                }],
                content: &ContentAutomaton {
                    transitions: &[&[("one", 1)], &[("two", 2)], &[]],
                    accepting: &[false, true, true],
                    expected: &[&["one"], &[], &[]],
                },
            },
            Variant {
                attributes: &[AttributeTerm {
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

    #[test]
    fn validate_first_variant() {
        assert_eq!(
            validate_rules(
                &create_element("example", vec![("foo", ""), ("bar", "")], vec!["one"]),
                &[],
                &[],
                &RULES,
            ),
            Ok(())
        );
    }

    #[test]
    fn validate_second_variant() {
        assert_eq!(
            validate_rules(
                &create_element("example", vec![("baz", "")], vec![]),
                &[],
                &[],
                &RULES,
            ),
            Ok(())
        );
    }

    #[test]
    fn validate_unknown_attribute() {
        assert_eq!(
            validate_rules(
                &create_element("example", vec![("foo", ""), ("unknown", "")], vec!["one"]),
                &[],
                &[],
                &RULES,
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
            validate_rules(
                &create_element("example", vec![("baz", ""), ("bar", "")], vec![]),
                &[],
                &[],
                &RULES,
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
            validate_rules(
                &create_element("example", vec![("bar", "")], vec!["one"]),
                &[],
                &[],
                &RULES,
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
            validate_rules(
                &create_element("example", vec![("foo", "")], vec!["two", "one"]),
                &[],
                &[],
                &RULES,
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
            validate_rules(
                &create_element("example", vec![("foo", "")], vec![]),
                &[],
                &[],
                &RULES,
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
            validate_rules(
                &create_element("example", vec![("foo", ""), ("data-x", "")], vec!["one"]),
                &[Regex::new("^data-.*$").unwrap()],
                &[],
                &RULES,
            ),
            Ok(())
        );
    }

    #[test]
    fn skip_ignored_child() {
        assert_eq!(
            validate_rules(
                &create_element("example", vec![("foo", "")], vec!["one", "custom-x"]),
                &[],
                &[Regex::new("^custom-.*$").unwrap()],
                &RULES,
            ),
            Ok(())
        );
    }

    #[test]
    fn satisfy_requirement_with_ignored_attribute() {
        assert_eq!(
            validate_rules(
                &create_element("example", vec![("foo", "")], vec!["one"]),
                &[Regex::new("^foo$").unwrap()],
                &[],
                &RULES,
            ),
            Ok(())
        );
    }

    #[test]
    fn satisfy_requirement_with_ignored_child() {
        assert_eq!(
            validate_rules(
                &create_element("example", vec![("foo", "")], vec!["one"]),
                &[],
                &[Regex::new("^one$").unwrap()],
                &RULES,
            ),
            Ok(())
        );
    }

    #[test]
    fn suppress_missing_ignored_attribute() {
        assert_eq!(
            validate_rules(
                &create_element("example", vec![], vec!["one"]),
                &[Regex::new("^foo$").unwrap()],
                &[],
                &RULES,
            ),
            Ok(())
        );
    }

    #[test]
    fn suppress_missing_ignored_child() {
        assert_eq!(
            validate_rules(
                &create_element("example", vec![("foo", "")], vec![]),
                &[],
                &[Regex::new("^one$").unwrap()],
                &RULES,
            ),
            Ok(())
        );
    }

    #[test]
    fn skip_misplaced_ignored_child() {
        assert_eq!(
            validate_rules(
                &create_element("example", vec![("foo", "")], vec!["two", "one", "two"]),
                &[],
                &[Regex::new("^two$").unwrap()],
                &RULES,
            ),
            Ok(())
        );
    }

    #[test]
    fn prefer_variant_with_fewer_conflicts() {
        // The second variant conflicts with the present `bar` attribute while
        // the first one misses the required `foo` attribute.
        assert_eq!(
            validate_rules(
                &create_element("example", vec![("bar", "")], vec![]),
                &[],
                &[],
                &RULES,
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
