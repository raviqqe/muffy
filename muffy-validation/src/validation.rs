mod state;
mod variant_evaluation;

use self::{state::State, variant_evaluation::VariantEvaluation};
use crate::{
    attribute_set::AttributeSet,
    content::{Content, TEXT_TOKEN},
    error::{AttributeError, ChildError, MarkupError},
    rule::Rule,
    variant::Variant,
};
use alloc::collections::{BTreeMap, BTreeSet};
use muffy_document::document::{Element, Node};
use regex::Regex;

const EMPTY_ATTRIBUTE_SET: AttributeSet = AttributeSet {
    required: &[],
    optional: &[],
};

pub fn validate_rule(
    element: &Element,
    ignored_attributes: &[Regex],
    ignored_elements: &[Regex],
    rule: &Rule,
) -> Result<(), MarkupError> {
    let (attributes, exempt_attributes, disallowed_attributes) =
        classify_attributes(element, ignored_attributes, rule);
    let (children, disallowed_children) = classify_children(element, ignored_elements, rule);

    let mut attribute_errors = disallowed_attributes
        .into_iter()
        .map(|name| (name.into(), [AttributeError::NotAllowed].into()))
        .collect::<BTreeMap<String, BTreeSet<AttributeError>>>();
    let mut child_errors = disallowed_children
        .into_iter()
        .map(|name| (name.into(), [ChildError::NotAllowed].into()))
        .collect::<BTreeMap<String, BTreeSet<ChildError>>>();

    let mut missing_attributes = BTreeSet::new();
    let mut missing_children = BTreeSet::new();

    if let Some(VariantEvaluation {
        attribute_set,
        misplaced_children,
        state,
        ..
    }) = rule
        .variants
        .iter()
        .map(|variant| evaluate_variant(variant, &attributes, &exempt_attributes, &children))
        .min_by_key(|evaluation| evaluation.score)
    {
        for (name, value) in &attributes {
            if let Some(attribute) = attribute_set.find(name) {
                if !attribute.value.matches(value) {
                    attribute_errors
                        .entry((*name).into())
                        .or_default()
                        .insert(AttributeError::InvalidValue);
                }
            } else {
                attribute_errors
                    .entry((*name).into())
                    .or_default()
                    .insert(AttributeError::Conflict);
            }
        }

        for name in &misplaced_children {
            child_errors
                .entry((*name).into())
                .or_default()
                .insert(ChildError::Misplaced);
        }

        missing_attributes.extend(
            attribute_set
                .required
                .iter()
                .filter(|attribute| {
                    attributes
                        .binary_search_by(|(name, _)| name.cmp(&attribute.name))
                        .is_err()
                        && exempt_attributes.binary_search(&attribute.name).is_err()
                        && !ignored_attributes
                            .iter()
                            .any(|pattern| pattern.is_match(attribute.name))
                })
                .map(|attribute| attribute.name.into()),
        );

        if misplaced_children.is_empty() {
            missing_children.extend(
                collect_missing_children(&state, rule.children)
                    .iter()
                    .filter(|name| {
                        !ignored_elements
                            .iter()
                            .any(|pattern| pattern.is_match(name))
                    })
                    .map(|&name| name.into()),
            );
        }
    }

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

fn validate_children(
    content: &'static Content,
    children: &[(&'static str, bool)],
) -> (BTreeSet<&'static str>, State) {
    let mut state = State::Content(content);
    let mut misplaced = BTreeSet::new();

    for (name, exempt) in children {
        let next = state.step(name);

        if next != State::NotAllowed {
            state = next;
        } else if !exempt {
            misplaced.insert(*name);
        }
    }

    (misplaced, state)
}

fn collect_missing_children(state: &State, names: &[&'static str]) -> BTreeSet<&'static str> {
    if state.is_nullable() {
        return Default::default();
    }

    let mut visited = BTreeSet::from([(state.clone(), None)]);
    let mut states = vec![(state.clone(), None)];

    loop {
        let mut expected = BTreeSet::new();
        let mut next_frontier = vec![];

        for (state, initial_name) in &states {
            for name in names {
                let next = state.step(name);

                if next == State::NotAllowed {
                    continue;
                }

                let name = initial_name.unwrap_or(name);

                if next.is_nullable() {
                    expected.insert(*name);
                }

                if visited.insert((next.clone(), Some(name))) {
                    next_frontier.push((next, Some(name)));
                }
            }
        }

        if !expected.is_empty() || next_frontier.is_empty() {
            return expected;
        }

        states = next_frontier;
    }
}

fn classify_attributes<'a>(
    element: &'a Element,
    ignored_attributes: &[Regex],
    rule: &Rule,
) -> (
    Vec<(&'static str, &'a str)>,
    Vec<&'static str>,
    Vec<&'a str>,
) {
    let mut attributes = vec![];
    let mut exempt_attributes = vec![];
    let mut disallowed_attributes = vec![];

    for (name, value) in element.attributes() {
        let ignored = ignored_attributes
            .iter()
            .any(|pattern| pattern.is_match(name));

        if let Some(name) = find_name(rule.attributes, name) {
            if ignored {
                exempt_attributes.push(name);
            } else {
                attributes.push((name, value));
            }
        } else if !ignored {
            disallowed_attributes.push(name);
        }
    }

    attributes.sort_by_key(|(name, _)| *name);
    attributes.dedup_by_key(|(name, _)| *name);
    exempt_attributes.sort();
    exempt_attributes.dedup();

    (attributes, exempt_attributes, disallowed_attributes)
}

fn classify_children<'a>(
    element: &'a Element,
    ignored_elements: &[Regex],
    rule: &Rule,
) -> (Vec<(&'static str, bool)>, Vec<&'a str>) {
    let mut children = vec![];
    let mut disallowed_children = vec![];

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
        let exempt = ignored_elements
            .iter()
            .any(|pattern| pattern.is_match(name));

        if let Some(name) = find_name(rule.children, name) {
            children.push((name, exempt));
        } else if !exempt {
            disallowed_children.push(name);
        }
    }

    (children, disallowed_children)
}

fn find_name(names: &'static [&'static str], name: &str) -> Option<&'static str> {
    if let Ok(index) = names.binary_search(&name) {
        Some(names[index])
    } else {
        names
            .iter()
            .copied()
            .find(|pattern| matches_wildcard(pattern, name))
    }
}

pub fn matches_wildcard(pattern: &str, name: &str) -> bool {
    match pattern.strip_suffix('*') {
        Some(":") => !name.contains(':'),
        Some(prefix) => name.starts_with(prefix),
        None => false,
    }
}

fn evaluate_variant(
    variant: &Variant,
    attributes: &[(&'static str, &str)],
    exempt_attributes: &[&'static str],
    children: &[(&'static str, bool)],
) -> VariantEvaluation {
    let (attribute_set, (error_count, conflict_count, requirement_count)) = variant
        .attributes
        .iter()
        .map(|set| {
            (
                set,
                evaluate_attribute_set(set, attributes, exempt_attributes),
            )
        })
        .min_by_key(|(_, score)| *score)
        .unwrap_or((&EMPTY_ATTRIBUTE_SET, Default::default()));
    let (misplaced_children, state) = validate_children(variant.content, children);

    VariantEvaluation {
        score: (
            error_count
                + misplaced_children.len()
                + usize::from(misplaced_children.is_empty() && !state.is_nullable()),
            conflict_count + misplaced_children.len(),
            requirement_count,
        ),
        attribute_set,
        misplaced_children,
        state,
    }
}

// (error count, conflict count, requirement count)
fn evaluate_attribute_set(
    set: &AttributeSet,
    attributes: &[(&'static str, &str)],
    exempt_attributes: &[&'static str],
) -> (usize, usize, usize) {
    let mut conflict_count = 0;
    let mut mismatch_count = 0;

    for (name, value) in attributes {
        match set.find(name) {
            Some(attribute) => {
                if !attribute.value.matches(value) {
                    mismatch_count += 1;
                }
            }
            None => conflict_count += 1,
        }
    }

    let missing_count = set
        .required
        .iter()
        .filter(|attribute| {
            attributes
                .binary_search_by(|(name, _)| name.cmp(&attribute.name))
                .is_err()
                && exempt_attributes.binary_search(&attribute.name).is_err()
        })
        .count();

    (
        conflict_count + mismatch_count + missing_count,
        conflict_count,
        set.required.len(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        attribute::Attribute,
        value::{Literal, Value},
    };
    use alloc::sync::Arc;
    use pretty_assertions::assert_eq;

    const EMPTY_CONTENT: Content = Content::Empty;

    const fn attribute(name: &'static str) -> Attribute {
        Attribute {
            name,
            value: &Value::Any,
        }
    }

    // The content model of `element example { (attribute foo { text },
    // attribute bar { text }?) | attribute baz { text } }` with children
    // `(one, two?)`.
    const RULE: Rule = Rule {
        attributes: &["bar", "baz", "foo"],
        children: &["one", "two"],
        variants: &[
            Variant {
                attributes: &[AttributeSet {
                    required: &[attribute("foo")],
                    optional: &[attribute("bar")],
                }],
                content: &Content::Group(&[
                    Content::Element(&["one"]),
                    Content::Optional(&Content::Element(&["two"])),
                ]),
            },
            Variant {
                attributes: &[AttributeSet {
                    required: &[attribute("baz")],
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
        let (misplaced, state) = validate_children(
            content,
            &names.iter().map(|&name| (name, false)).collect::<Vec<_>>(),
        );

        misplaced.is_empty() && state.is_nullable()
    }

    mod wildcard {
        use super::*;

        #[test]
        fn match_any_name() {
            assert!(matches_wildcard("*", "zoom"));
            assert!(matches_wildcard("*", "inkscape:zoom"));
        }

        #[test]
        fn match_prefixed_names() {
            assert!(matches_wildcard("inkscape:*", "inkscape:zoom"));
            assert!(!matches_wildcard("inkscape:*", "sodipodi:zoom"));
            assert!(!matches_wildcard("inkscape:*", "zoom"));
        }

        #[test]
        fn match_names_in_no_namespace() {
            assert!(matches_wildcard(":*", "zoom"));
            assert!(!matches_wildcard(":*", "inkscape:zoom"));
            // A no-namespace name containing a colon is conservatively
            // rejected.
            assert!(!matches_wildcard(":*", "xmlns:foo"));
        }

        #[test]
        fn match_no_plain_name() {
            assert!(!matches_wildcard("zoom", "zoom"));
        }
    }

    mod content {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn accept_ordered_group() {
            const CONTENT: Content =
                Content::Group(&[Content::Element(&["foo"]), Content::Element(&["bar"])]);

            assert!(accepts(&CONTENT, &["foo", "bar"]));
            assert!(!accepts(&CONTENT, &["bar", "foo"]));
            assert!(!accepts(&CONTENT, &["foo"]));
            assert!(!accepts(&CONTENT, &["foo", "bar", "foo"]));
        }

        #[test]
        fn accept_optional_operand() {
            const CONTENT: Content = Content::Group(&[
                Content::Optional(&Content::Element(&["foo"])),
                Content::Element(&["bar"]),
            ]);

            assert!(accepts(&CONTENT, &["bar"]));
            assert!(accepts(&CONTENT, &["foo", "bar"]));
            assert!(!accepts(&CONTENT, &["foo"]));
        }

        #[test]
        fn accept_interleave_in_any_order() {
            const CONTENT: Content =
                Content::Interleave(&[Content::Element(&["foo"]), Content::Element(&["bar"])]);

            assert!(accepts(&CONTENT, &["foo", "bar"]));
            assert!(accepts(&CONTENT, &["bar", "foo"]));
            assert!(!accepts(&CONTENT, &["foo"]));
        }

        #[test]
        fn accept_repetition() {
            const CONTENT: Content = Content::Many0(&Content::Element(&["foo"]));

            assert!(accepts(&CONTENT, &[]));
            assert!(accepts(&CONTENT, &["foo", "foo", "foo"]));
            assert!(!accepts(&CONTENT, &["bar"]));
        }

        #[test]
        fn accept_at_least_one_repetition() {
            const CONTENT: Content = Content::Many1(&Content::Element(&["foo"]));

            assert!(!accepts(&CONTENT, &[]));
            assert!(accepts(&CONTENT, &["foo"]));
            assert!(accepts(&CONTENT, &["foo", "foo"]));
        }

        #[test]
        fn accept_repetition_interleaved_with_group() {
            const CONTENT: Content = Content::Interleave(&[
                Content::Group(&[Content::Element(&["foo"]), Content::Element(&["bar"])]),
                Content::Many0(&Content::Element(&["baz"])),
            ]);

            assert!(accepts(&CONTENT, &["baz", "foo", "baz", "bar", "baz"]));
            assert!(accepts(&CONTENT, &["foo", "bar"]));
            assert!(!accepts(&CONTENT, &["bar", "baz", "foo"]));
        }

        #[test]
        fn accept_text() {
            const CONTENT: Content = Content::Many0(&Content::Choice(&[
                Content::Text,
                Content::Element(&["foo"]),
            ]));

            assert!(accepts(&CONTENT, &[]));
            assert!(accepts(&CONTENT, &["foo", "foo"]));
            assert!(accepts(&CONTENT, &["#text", "foo", "#text"]));
        }

        #[test]
        fn expect_names_on_shortest_accepting_path() {
            const CONTENT: Content = Content::Interleave(&[
                Content::Group(&[Content::Element(&["foo"]), Content::Element(&["bar"])]),
                Content::Many0(&Content::Element(&["baz"])),
            ]);
            let names = ["bar", "baz", "foo"];

            let state = State::Content(&CONTENT);

            assert_eq!(collect_missing_children(&state, &names), ["foo"].into());

            let state = state.step("foo");

            assert_eq!(collect_missing_children(&state, &names), ["bar"].into());

            let state = state.step("bar");

            assert!(state.is_nullable());
            assert_eq!(collect_missing_children(&state, &names), [].into());
        }

        #[test]
        fn expect_alternative_children() {
            const CONTENT: Content =
                Content::Choice(&[Content::Element(&["bar"]), Content::Element(&["foo"])]);

            assert_eq!(
                collect_missing_children(&State::Content(&CONTENT), &["bar", "foo"]),
                ["bar", "foo"].into()
            );
        }

        #[test]
        fn expect_all_required_children() {
            const CONTENT: Content =
                Content::Interleave(&[Content::Element(&["bar"]), Content::Element(&["foo"])]);

            assert_eq!(
                collect_missing_children(&State::Content(&CONTENT), &["bar", "foo"]),
                ["bar", "foo"].into()
            );
        }

        #[test]
        fn expect_remaining_required_children() {
            const CONTENT: Content = Content::Interleave(&[
                Content::Element(&["bar"]),
                Content::Element(&["baz"]),
                Content::Element(&["foo"]),
            ]);

            let state = State::Content(&CONTENT).step("foo");

            assert_eq!(
                collect_missing_children(&state, &["bar", "baz", "foo"]),
                ["bar", "baz"].into()
            );
        }

        #[test]
        fn expect_alternatives_converging_on_a_shared_suffix() {
            // Both alternatives reach the same "needs foo" state, so both
            // starting children are reported despite the convergence.
            const CONTENT: Content = Content::Choice(&[
                Content::Group(&[Content::Element(&["bar"]), Content::Element(&["foo"])]),
                Content::Group(&[Content::Element(&["baz"]), Content::Element(&["foo"])]),
            ]);

            assert_eq!(
                collect_missing_children(&State::Content(&CONTENT), &["bar", "baz", "foo"]),
                ["bar", "baz"].into()
            );
        }
    }

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

    #[test]
    fn prefer_attribute_set_with_fewer_errors() {
        // The attributes of `element example { attribute bar { text }
        // | (attribute foo { text }, attribute qux { text }) }`.
        const ATTRIBUTE_SET_RULE: Rule = Rule {
            attributes: &["bar", "foo", "qux"],
            children: &[],
            variants: &[Variant {
                attributes: &[
                    AttributeSet {
                        required: &[attribute("bar")],
                        optional: &[],
                    },
                    AttributeSet {
                        required: &[attribute("foo"), attribute("qux")],
                        optional: &[],
                    },
                ],
                content: &EMPTY_CONTENT,
            }],
        };

        assert_eq!(
            validate_rule(
                &create_element("example", vec![("foo", "")], vec![]),
                &[],
                &[],
                &ATTRIBUTE_SET_RULE,
            ),
            Err(MarkupError::InvalidElement {
                invalid_attributes: Default::default(),
                invalid_children: Default::default(),
                missing_attributes: ["qux".into()].into(),
                missing_children: Default::default(),
            })
        );

        // The best set is chosen regardless of its position.
        assert_eq!(
            validate_rule(
                &create_element("example", vec![("bar", "")], vec![]),
                &[],
                &[],
                &ATTRIBUTE_SET_RULE,
            ),
            Ok(())
        );
    }

    mod value {
        use super::*;
        use pretty_assertions::assert_eq;

        // The attributes of `element example { attribute kind { "one" | "two" }? }`.
        const VALUE_RULE: Rule = Rule {
            attributes: &["kind"],
            children: &[],
            variants: &[Variant {
                attributes: &[AttributeSet {
                    required: &[],
                    optional: &[Attribute {
                        name: "kind",
                        value: &Value::LiteralSet(&[Literal::Exact("one"), Literal::Exact("two")]),
                    }],
                }],
                content: &EMPTY_CONTENT,
            }],
        };

        #[test]
        fn validate_valid_value() {
            assert_eq!(
                validate_rule(
                    &create_element("example", vec![("kind", "one")], vec![]),
                    &[],
                    &[],
                    &VALUE_RULE,
                ),
                Ok(())
            );
        }

        #[test]
        fn validate_invalid_value() {
            assert_eq!(
                validate_rule(
                    &create_element("example", vec![("kind", "three")], vec![]),
                    &[],
                    &[],
                    &VALUE_RULE,
                ),
                Err(MarkupError::InvalidElement {
                    invalid_attributes: [("kind".into(), [AttributeError::InvalidValue].into())]
                        .into(),
                    invalid_children: Default::default(),
                    missing_attributes: Default::default(),
                    missing_children: Default::default(),
                })
            );
        }

        #[test]
        fn skip_value_of_ignored_attribute() {
            assert_eq!(
                validate_rule(
                    &create_element("example", vec![("kind", "three")], vec![]),
                    &[Regex::new("^kind$").unwrap()],
                    &[],
                    &VALUE_RULE,
                ),
                Ok(())
            );
        }

        #[test]
        fn prefer_variant_with_matching_value() {
            // The rule of `element example { (attribute kind { "one" }, one)
            // | (attribute kind { "two" }, two) }`.
            const VARIANT_RULE: Rule = Rule {
                attributes: &["kind"],
                children: &["one", "two"],
                variants: &[
                    Variant {
                        attributes: &[AttributeSet {
                            required: &[Attribute {
                                name: "kind",
                                value: &Value::LiteralSet(&[Literal::Exact("one")]),
                            }],
                            optional: &[],
                        }],
                        content: &Content::Element(&["one"]),
                    },
                    Variant {
                        attributes: &[AttributeSet {
                            required: &[Attribute {
                                name: "kind",
                                value: &Value::LiteralSet(&[Literal::Exact("two")]),
                            }],
                            optional: &[],
                        }],
                        content: &Content::Element(&["two"]),
                    },
                ],
            };

            assert_eq!(
                validate_rule(
                    &create_element("example", vec![("kind", "two")], vec!["two"]),
                    &[],
                    &[],
                    &VARIANT_RULE,
                ),
                Ok(())
            );

            // The missing child is diagnosed against the variant with the
            // matching value.
            assert_eq!(
                validate_rule(
                    &create_element("example", vec![("kind", "two")], vec![]),
                    &[],
                    &[],
                    &VARIANT_RULE,
                ),
                Err(MarkupError::InvalidElement {
                    invalid_attributes: Default::default(),
                    invalid_children: Default::default(),
                    missing_attributes: Default::default(),
                    missing_children: ["two".into()].into(),
                })
            );
        }
    }
}
