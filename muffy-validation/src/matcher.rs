use crate::error::{AttributeError, ChildError, MarkupError};
use alloc::collections::{BTreeMap, BTreeSet};
use muffy_document::html::{Element, Node};
use regex::Regex;

/// One alternative of attribute names an element accepts: all required names
/// must be present, and no name outside the required and optional ones may be.
pub struct AttributeTerm {
    pub required: &'static [&'static str],
    pub optional: &'static [&'static str],
}

/// A deterministic automaton over child element names.
pub struct ContentAutomaton {
    /// Transitions sorted by child element name for each state.
    pub transitions: &'static [&'static [(&'static str, usize)]],
    /// Whether each state accepts the end of content.
    pub accepting: &'static [bool],
    /// Child element names on shortest paths from each state to acceptance.
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

#[derive(Default)]
struct VariantOutcome {
    conflicting: BTreeSet<&'static str>,
    missing_attributes: BTreeSet<&'static str>,
    misplaced: BTreeSet<&'static str>,
    missing_children: BTreeSet<&'static str>,
    // The reported sets union alternative diagnoses, so scores are tracked
    // separately from their sizes.
    attribute_error_count: usize,
    attribute_conflict_count: usize,
    requirement_count: usize,
}

impl VariantOutcome {
    // Errors on present names break ties so that a variant missing a name is
    // preferred over one conflicting with an equal number of present names,
    // and the requirement count so that the simplest variant is reported.
    fn error_count(&self) -> (usize, usize, usize) {
        (
            self.attribute_error_count
                + self.misplaced.len()
                + usize::from(!self.missing_children.is_empty()),
            self.attribute_conflict_count + self.misplaced.len(),
            self.requirement_count,
        )
    }
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

    for (name, _) in element.attributes() {
        if ignored_attributes
            .iter()
            .any(|pattern| pattern.is_match(name))
        {
            continue;
        }

        // Keep the static name so that variant outcomes stay allocation-free.
        if let Ok(index) = rules.attributes.binary_search(&name) {
            attributes.push(rules.attributes[index]);
        } else {
            attribute_errors
                .entry(name.into())
                .or_default()
                .insert(AttributeError::NotAllowed);
        }
    }

    attributes.sort_unstable();

    let mut children = vec![];

    for child in element.children() {
        if let Node::Element(child) = child {
            let name = child.name();

            if ignored_elements
                .iter()
                .any(|pattern| pattern.is_match(name))
            {
                continue;
            }

            if let Ok(index) = rules.children.binary_search(&name) {
                children.push(rules.children[index]);
            } else {
                child_errors
                    .entry(name.into())
                    .or_default()
                    .insert(ChildError::NotAllowed);
            }
        }
    }

    let outcome = rules
        .variants
        .iter()
        .map(|variant| evaluate_variant(variant, &attributes, &children))
        .min_by_key(VariantOutcome::error_count)
        .unwrap_or_default();

    for name in outcome.conflicting {
        attribute_errors
            .entry(name.into())
            .or_default()
            .insert(AttributeError::Conflicting);
    }

    for name in outcome.misplaced {
        child_errors
            .entry(name.into())
            .or_default()
            .insert(ChildError::Misplaced);
    }

    if attribute_errors.is_empty()
        && child_errors.is_empty()
        && outcome.missing_attributes.is_empty()
        && outcome.missing_children.is_empty()
    {
        Ok(())
    } else {
        Err(MarkupError::InvalidElement {
            attributes: attribute_errors,
            children: child_errors,
            missing_attributes: outcome
                .missing_attributes
                .into_iter()
                .map(Into::into)
                .collect(),
            missing_children: outcome
                .missing_children
                .into_iter()
                .map(Into::into)
                .collect(),
        })
    }
}

fn evaluate_variant(
    variant: &Variant,
    attributes: &[&'static str],
    children: &[&'static str],
) -> VariantOutcome {
    let terms = variant
        .attributes
        .iter()
        .map(|term| {
            (
                attributes
                    .iter()
                    .filter(|name| {
                        term.required.binary_search(name).is_err()
                            && term.optional.binary_search(name).is_err()
                    })
                    .copied()
                    .collect::<BTreeSet<_>>(),
                term.required
                    .iter()
                    .filter(|name| attributes.binary_search(name).is_err())
                    .copied()
                    .collect::<BTreeSet<_>>(),
                term.required.len(),
            )
        })
        .collect::<Vec<_>>();
    // Equally scored terms are alternative diagnoses, so report their union.
    let minimum = terms
        .iter()
        .map(|(conflicting, missing, requirement_count)| {
            (conflicting.len() + missing.len(), *requirement_count)
        })
        .min()
        .unwrap_or_default();
    let tied = terms
        .into_iter()
        .filter(|(conflicting, missing, requirement_count)| {
            (conflicting.len() + missing.len(), *requirement_count) == minimum
        })
        .collect::<Vec<_>>();
    let attribute_conflict_count = tied
        .iter()
        .map(|(conflicting, _, _)| conflicting.len())
        .min()
        .unwrap_or_default();
    let (conflicting, missing_attributes) = tied.into_iter().fold(
        Default::default(),
        |(all_conflicting, all_missing): (BTreeSet<_>, BTreeSet<_>), (conflicting, missing, _)| {
            (
                all_conflicting.into_iter().chain(conflicting).collect(),
                all_missing.into_iter().chain(missing).collect(),
            )
        },
    );

    let mut state = 0;
    let mut misplaced = BTreeSet::new();

    for name in children {
        if let Ok(index) =
            variant.content.transitions[state].binary_search_by_key(name, |(name, _)| name)
        {
            state = variant.content.transitions[state][index].1;
        } else {
            misplaced.insert(*name);
        }
    }

    VariantOutcome {
        conflicting,
        missing_attributes,
        missing_children: if misplaced.is_empty() && !variant.content.accepting[state] {
            variant.content.expected[state].iter().copied().collect()
        } else {
            Default::default()
        },
        misplaced,
        attribute_error_count: minimum.0,
        attribute_conflict_count,
        requirement_count: minimum.1,
    }
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
                attributes: [("unknown".into(), [AttributeError::NotAllowed].into())].into(),
                children: Default::default(),
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
                attributes: [("bar".into(), [AttributeError::Conflicting].into())].into(),
                children: Default::default(),
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
                attributes: Default::default(),
                children: Default::default(),
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
                attributes: Default::default(),
                children: [("two".into(), [ChildError::Misplaced].into())].into(),
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
                attributes: Default::default(),
                children: Default::default(),
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
}
