use super::Pattern;
use crate::error::MacroError;

pub fn normalize_pattern(pattern: &Pattern) -> Result<Vec<(Pattern, Pattern)>, MacroError> {
    Ok(match pattern {
        Pattern::Attribute(_) => vec![(pattern.clone(), Pattern::Empty)],
        Pattern::Element(_) | Pattern::Text => {
            vec![(Pattern::Empty, pattern.clone())]
        }
        Pattern::Empty => vec![(Pattern::Empty, Pattern::Empty)],
        Pattern::NotAllowed => vec![],
        Pattern::Group(patterns) | Pattern::Interleave(patterns) => {
            let interleaved = matches!(pattern, Pattern::Interleave(_));
            let mut variants = vec![(Pattern::Empty, Pattern::Empty)];

            for pattern in patterns {
                let others = normalize_pattern(pattern)?;

                variants = variants
                    .iter()
                    .flat_map(|(attribute, content)| {
                        others.iter().map(|(other_attribute, other_content)| {
                            (
                                Pattern::interleave([attribute.clone(), other_attribute.clone()]),
                                if interleaved {
                                    Pattern::interleave
                                } else {
                                    Pattern::group
                                }([
                                    content.clone(),
                                    other_content.clone(),
                                ]),
                            )
                        })
                    })
                    .collect();
            }

            variants
        }
        Pattern::Choice(patterns) => {
            let variants = patterns
                .iter()
                .map(normalize_pattern)
                .collect::<Result<Vec<_>, _>>()?;

            if variants
                .iter()
                .flatten()
                .all(|(_, content)| *content == Pattern::Empty)
            {
                vec![(
                    Pattern::choice(
                        variants
                            .into_iter()
                            .flatten()
                            .map(|(attribute, _)| attribute),
                    ),
                    Pattern::Empty,
                )]
            } else if variants
                .iter()
                .flatten()
                .all(|(attribute, _)| *attribute == Pattern::Empty)
            {
                vec![(
                    Pattern::Empty,
                    Pattern::choice(variants.into_iter().flatten().map(|(_, content)| content)),
                )]
            } else {
                variants.into_iter().flatten().collect()
            }
        }
        Pattern::Optional(pattern) => {
            let variants = normalize_pattern(pattern)?;

            match variants.as_slice() {
                [(attribute, content)] if *content == Pattern::Empty => {
                    vec![(Pattern::optional(attribute.clone()), Pattern::Empty)]
                }
                [(attribute, content)] if *attribute == Pattern::Empty => {
                    vec![(Pattern::Empty, Pattern::optional(content.clone()))]
                }
                _ => [(Pattern::Empty, Pattern::Empty)]
                    .into_iter()
                    .chain(variants)
                    .collect(),
            }
        }
        Pattern::Many0(operand) | Pattern::Many1(operand) => {
            let many1 = matches!(pattern, Pattern::Many1(_));
            let mut attributes = vec![];
            let mut contents = vec![];

            for (attribute, content) in normalize_pattern(operand)? {
                // Groups of attributes and contents in repetitions are prohibited in Relax NG.
                if attribute != Pattern::Empty && content != Pattern::Empty {
                    return Err(MacroError::RncPattern(
                        "attribute grouped with content in a repetition",
                    ));
                }

                if attribute != Pattern::Empty {
                    attributes.push(attribute);
                } else if content != Pattern::Empty {
                    contents.push(content);
                }
            }

            let attributes_empty = attributes.is_empty();
            let contents_empty = contents.is_empty();

            vec![(
                if attributes_empty {
                    Pattern::Empty
                } else if many1 && contents_empty {
                    Pattern::choice(attributes)
                } else {
                    Pattern::optional(Pattern::choice(attributes))
                },
                if contents_empty {
                    Pattern::Empty
                } else if many1 && attributes_empty {
                    Pattern::many1(Pattern::choice(contents))
                } else {
                    Pattern::many0(Pattern::choice(contents))
                },
            )]
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn attribute(name: &str) -> Pattern {
        Pattern::Attribute([name.into()].into())
    }

    fn element(name: &str) -> Pattern {
        Pattern::Element([name.into()].into())
    }

    #[test]
    fn split_attribute_and_element() {
        assert_eq!(
            normalize_pattern(&Pattern::interleave([attribute("foo"), element("bar")])).unwrap(),
            vec![(attribute("foo"), element("bar"))]
        );
    }

    #[test]
    fn keep_attribute_choice_in_one_alternative() {
        assert_eq!(
            normalize_pattern(&Pattern::choice([attribute("foo"), attribute("bar")])).unwrap(),
            vec![(
                Pattern::choice([attribute("foo"), attribute("bar")]),
                Pattern::Empty
            )]
        );
    }

    #[test]
    fn lift_mixed_choice_into_alternatives() {
        assert_eq!(
            normalize_pattern(&Pattern::choice([attribute("foo"), element("bar")])).unwrap(),
            vec![
                (attribute("foo"), Pattern::Empty),
                (Pattern::Empty, element("bar")),
            ]
        );
    }

    #[test]
    fn split_optional_attribute() {
        assert_eq!(
            normalize_pattern(&Pattern::optional(attribute("foo"))).unwrap(),
            vec![(Pattern::optional(attribute("foo")), Pattern::Empty)]
        );
    }

    #[test]
    fn split_element_repetition() {
        assert_eq!(
            normalize_pattern(&Pattern::many0(element("foo"))).unwrap(),
            vec![(Pattern::Empty, Pattern::many0(element("foo")))]
        );
    }

    #[test]
    fn split_at_least_one_element_repetition() {
        assert_eq!(
            normalize_pattern(&Pattern::many1(element("foo"))).unwrap(),
            vec![(Pattern::Empty, Pattern::many1(element("foo")))]
        );
    }

    #[test]
    fn require_one_of_repeated_attributes() {
        assert_eq!(
            normalize_pattern(&Pattern::many1(Pattern::choice([
                attribute("foo"),
                attribute("bar")
            ])))
            .unwrap(),
            vec![(
                Pattern::choice([attribute("foo"), attribute("bar")]),
                Pattern::Empty
            )]
        );
    }

    #[test]
    fn fail_on_attribute_grouped_with_element_in_repetition() {
        assert!(matches!(
            normalize_pattern(&Pattern::many1(Pattern::group([
                attribute("foo"),
                element("bar")
            ]))),
            Err(MacroError::RncPattern(_))
        ));
    }

    #[test]
    fn fail_on_chosen_attribute_groups_in_repetition() {
        assert!(matches!(
            normalize_pattern(&Pattern::many1(Pattern::choice([
                Pattern::group([attribute("foo"), element("bar")]),
                Pattern::group([attribute("baz"), element("qux")])
            ]))),
            Err(MacroError::RncPattern(_))
        ));
    }

    #[test]
    fn split_repeated_attribute_and_element() {
        assert_eq!(
            normalize_pattern(&Pattern::many0(Pattern::choice([
                attribute("foo"),
                element("bar")
            ])))
            .unwrap(),
            vec![(
                Pattern::optional(attribute("foo")),
                Pattern::many0(element("bar"))
            )]
        );
    }

    #[test]
    fn split_not_allowed_into_no_alternative() {
        assert_eq!(normalize_pattern(&Pattern::NotAllowed).unwrap(), vec![]);
    }
}
