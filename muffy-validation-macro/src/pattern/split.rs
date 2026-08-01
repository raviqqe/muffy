use super::ResolvedPattern;
use crate::error::MacroError;
use alloc::collections::BTreeSet;

pub fn split_pattern(
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
                let others = split_pattern(pattern)?;

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
                .map(split_pattern)
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
            let variants = split_pattern(pattern)?;

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
            let at_least_once = matches!(pattern, ResolvedPattern::Many1(_));
            let variants = split_pattern(operand)?;

            if variants
                .iter()
                .all(|(attribute, _)| *attribute == ResolvedPattern::Empty)
            {
                let content =
                    ResolvedPattern::choice(variants.into_iter().map(|(_, content)| content));

                vec![(
                    ResolvedPattern::Empty,
                    if at_least_once {
                        ResolvedPattern::many1(content)
                    } else {
                        ResolvedPattern::many0(content)
                    },
                )]
            } else if variants
                .iter()
                .all(|(_, content)| *content == ResolvedPattern::Empty)
            {
                // Iterations of a repetition match alternatives independently
                // while attribute names never repeat on an element, so a
                // repetition accepts any combination of the attribute names.
                //
                // TODO Require at least one attribute for one-or-more repetitions.
                vec![(
                    ResolvedPattern::interleave(
                        variants
                            .iter()
                            .flat_map(|(attribute, _)| attribute_names(attribute))
                            .collect::<BTreeSet<_>>()
                            .into_iter()
                            .map(|name| {
                                ResolvedPattern::optional(ResolvedPattern::Attribute([name].into()))
                            }),
                    ),
                    ResolvedPattern::Empty,
                )]
            } else {
                return Err(MacroError::RncPattern("repeated mixed pattern"));
            }
        }
    })
}

fn attribute_names(pattern: &ResolvedPattern) -> BTreeSet<String> {
    match pattern {
        ResolvedPattern::Attribute(names) => names.clone(),
        ResolvedPattern::Choice(patterns)
        | ResolvedPattern::Group(patterns)
        | ResolvedPattern::Interleave(patterns) => {
            patterns.iter().flat_map(attribute_names).collect()
        }
        ResolvedPattern::Many0(pattern)
        | ResolvedPattern::Many1(pattern)
        | ResolvedPattern::Optional(pattern) => attribute_names(pattern),
        ResolvedPattern::Element(_)
        | ResolvedPattern::Empty
        | ResolvedPattern::NotAllowed
        | ResolvedPattern::Text => Default::default(),
    }
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
            split_pattern(&ResolvedPattern::interleave([
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
            split_pattern(&ResolvedPattern::choice([
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
            split_pattern(&ResolvedPattern::choice([attribute("foo"), element("bar")])).unwrap(),
            vec![
                (attribute("foo"), ResolvedPattern::Empty),
                (ResolvedPattern::Empty, element("bar")),
            ]
        );
    }

    #[test]
    fn split_optional_attribute() {
        assert_eq!(
            split_pattern(&ResolvedPattern::optional(attribute("foo"))).unwrap(),
            vec![(
                ResolvedPattern::optional(attribute("foo")),
                ResolvedPattern::Empty
            )]
        );
    }

    #[test]
    fn split_element_repetition() {
        assert_eq!(
            split_pattern(&ResolvedPattern::many0(element("foo"))).unwrap(),
            vec![(
                ResolvedPattern::Empty,
                ResolvedPattern::many0(element("foo"))
            )]
        );
    }

    #[test]
    fn split_attribute_choice_repetition_into_optional_attributes() {
        assert_eq!(
            split_pattern(&ResolvedPattern::many1(ResolvedPattern::choice([
                attribute("foo"),
                attribute("bar")
            ])))
            .unwrap(),
            vec![(
                ResolvedPattern::interleave([
                    ResolvedPattern::optional(attribute("bar")),
                    ResolvedPattern::optional(attribute("foo")),
                ]),
                ResolvedPattern::Empty
            )]
        );
    }

    #[test]
    fn split_not_allowed_into_no_alternative() {
        assert_eq!(split_pattern(&ResolvedPattern::NotAllowed).unwrap(), vec![]);
    }
}
