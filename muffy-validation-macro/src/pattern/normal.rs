use super::ResolvedPattern;
use crate::error::MacroError;

pub fn normalize_pattern(
    pattern: &ResolvedPattern,
) -> Result<Vec<(ResolvedPattern, ResolvedPattern)>, MacroError> {
    Ok(match pattern {
        ResolvedPattern::Attribute(_) => vec![(pattern.clone(), ResolvedPattern::Empty)],
        ResolvedPattern::Element(_) | ResolvedPattern::Text => {
            vec![(ResolvedPattern::Empty, pattern.clone())]
        }
        ResolvedPattern::Empty => vec![(ResolvedPattern::Empty, ResolvedPattern::Empty)],
        ResolvedPattern::NotAllowed => vec![],
        ResolvedPattern::Group(patterns) | ResolvedPattern::Interleave(patterns) => {
            let interleaved = matches!(pattern, ResolvedPattern::Interleave(_));
            let mut variants = vec![(ResolvedPattern::Empty, ResolvedPattern::Empty)];

            for pattern in patterns {
                let others = normalize_pattern(pattern)?;

                variants = variants
                    .iter()
                    .flat_map(|(attribute, content)| {
                        others.iter().map(|(other_attribute, other_content)| {
                            (
                                ResolvedPattern::interleave([
                                    attribute.clone(),
                                    other_attribute.clone(),
                                ]),
                                if interleaved {
                                    ResolvedPattern::interleave
                                } else {
                                    ResolvedPattern::group
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
        ResolvedPattern::Choice(patterns) => {
            let variants = patterns
                .iter()
                .map(normalize_pattern)
                .collect::<Result<Vec<_>, _>>()?;

            if variants
                .iter()
                .flatten()
                .all(|(_, content)| *content == ResolvedPattern::Empty)
            {
                vec![(
                    ResolvedPattern::choice(
                        variants
                            .into_iter()
                            .flatten()
                            .map(|(attribute, _)| attribute),
                    ),
                    ResolvedPattern::Empty,
                )]
            } else if variants
                .iter()
                .flatten()
                .all(|(attribute, _)| *attribute == ResolvedPattern::Empty)
            {
                vec![(
                    ResolvedPattern::Empty,
                    ResolvedPattern::choice(
                        variants.into_iter().flatten().map(|(_, content)| content),
                    ),
                )]
            } else {
                variants.into_iter().flatten().collect()
            }
        }
        ResolvedPattern::Optional(pattern) => {
            let variants = normalize_pattern(pattern)?;

            match variants.as_slice() {
                [(attribute, content)] if *content == ResolvedPattern::Empty => {
                    vec![(
                        ResolvedPattern::optional(attribute.clone()),
                        ResolvedPattern::Empty,
                    )]
                }
                [(attribute, content)] if *attribute == ResolvedPattern::Empty => {
                    vec![(
                        ResolvedPattern::Empty,
                        ResolvedPattern::optional(content.clone()),
                    )]
                }
                _ => [(ResolvedPattern::Empty, ResolvedPattern::Empty)]
                    .into_iter()
                    .chain(variants)
                    .collect(),
            }
        }
        ResolvedPattern::Many0(operand) | ResolvedPattern::Many1(operand) => {
            let variants = normalize_pattern(operand)?;

            if !variants
                .iter()
                .all(|(attribute, _)| *attribute == ResolvedPattern::Empty)
            {
                // TODO Support attribute patterns in repetitions.
                return Err(MacroError::RncPattern("repeated attribute pattern"));
            }

            vec![(
                ResolvedPattern::Empty,
                if matches!(pattern, ResolvedPattern::Many1(_)) {
                    ResolvedPattern::many1
                } else {
                    ResolvedPattern::many0
                }(ResolvedPattern::choice(
                    variants.into_iter().map(|(_, content)| content),
                )),
            )]
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn attribute(name: &str) -> ResolvedPattern {
        ResolvedPattern::Attribute([name.into()].into())
    }

    fn element(name: &str) -> ResolvedPattern {
        ResolvedPattern::Element([name.into()].into())
    }

    #[test]
    fn split_attribute_and_element() {
        assert_eq!(
            normalize_pattern(&ResolvedPattern::interleave([
                attribute("foo"),
                element("bar")
            ]))
            .unwrap(),
            vec![(attribute("foo"), element("bar"))]
        );
    }

    #[test]
    fn keep_attribute_choice_in_one_alternative() {
        assert_eq!(
            normalize_pattern(&ResolvedPattern::choice([
                attribute("foo"),
                attribute("bar")
            ]))
            .unwrap(),
            vec![(
                ResolvedPattern::choice([attribute("foo"), attribute("bar")]),
                ResolvedPattern::Empty
            )]
        );
    }

    #[test]
    fn lift_mixed_choice_into_alternatives() {
        assert_eq!(
            normalize_pattern(&ResolvedPattern::choice([attribute("foo"), element("bar")]))
                .unwrap(),
            vec![
                (attribute("foo"), ResolvedPattern::Empty),
                (ResolvedPattern::Empty, element("bar")),
            ]
        );
    }

    #[test]
    fn split_optional_attribute() {
        assert_eq!(
            normalize_pattern(&ResolvedPattern::optional(attribute("foo"))).unwrap(),
            vec![(
                ResolvedPattern::optional(attribute("foo")),
                ResolvedPattern::Empty
            )]
        );
    }

    #[test]
    fn split_element_repetition() {
        assert_eq!(
            normalize_pattern(&ResolvedPattern::many0(element("foo"))).unwrap(),
            vec![(
                ResolvedPattern::Empty,
                ResolvedPattern::many0(element("foo"))
            )]
        );
    }

    #[test]
    fn split_at_least_one_element_repetition() {
        assert_eq!(
            normalize_pattern(&ResolvedPattern::many1(element("foo"))).unwrap(),
            vec![(
                ResolvedPattern::Empty,
                ResolvedPattern::many1(element("foo"))
            )]
        );
    }

    #[test]
    fn fail_on_attribute_repetition() {
        assert!(matches!(
            normalize_pattern(&ResolvedPattern::many1(ResolvedPattern::choice([
                attribute("foo"),
                attribute("bar")
            ]))),
            Err(MacroError::RncPattern(_))
        ));
    }

    #[test]
    fn split_not_allowed_into_no_alternative() {
        assert_eq!(
            normalize_pattern(&ResolvedPattern::NotAllowed).unwrap(),
            vec![]
        );
    }
}
