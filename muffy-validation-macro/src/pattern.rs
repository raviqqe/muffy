mod resolve;

pub use self::resolve::resolve_pattern;
use crate::error::MacroError;
use alloc::collections::BTreeSet;
use muffy_rnc::{Identifier, NameClass};

// TODO Support attribute value schemas.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ResolvedPattern {
    Attribute(BTreeSet<String>),
    Choice(Vec<Self>),
    Element(BTreeSet<String>),
    Empty,
    Group(Vec<Self>),
    Interleave(Vec<Self>),
    Many0(Box<Self>),
    Many1(Box<Self>),
    NotAllowed,
    Optional(Box<Self>),
    Text,
}

impl ResolvedPattern {
    pub fn choice(patterns: impl IntoIterator<Item = Self>) -> Self {
        let mut alternatives = BTreeSet::new();
        let mut nullable = false;

        for pattern in patterns {
            match pattern {
                Self::NotAllowed => {}
                Self::Empty => nullable = true,
                Self::Choice(patterns) => alternatives.extend(patterns),
                pattern => {
                    alternatives.insert(pattern);
                }
            }
        }

        let pattern = if alternatives.is_empty() {
            Self::NotAllowed
        } else if alternatives.len() == 1 {
            alternatives.pop_first().expect("alternative")
        } else {
            Self::Choice(alternatives.into_iter().collect())
        };

        if nullable {
            Self::optional(pattern)
        } else {
            pattern
        }
    }

    pub fn group(patterns: impl IntoIterator<Item = Self>) -> Self {
        let mut sequence = vec![];

        for pattern in patterns {
            match pattern {
                Self::Empty => {}
                Self::NotAllowed => return Self::NotAllowed,
                Self::Group(patterns) => sequence.extend(patterns),
                pattern => sequence.push(pattern),
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

    pub fn interleave(patterns: impl IntoIterator<Item = Self>) -> Self {
        let mut operands = vec![];

        for pattern in patterns {
            match pattern {
                Self::Empty => {}
                Self::NotAllowed => return Self::NotAllowed,
                Self::Interleave(patterns) => operands.extend(patterns),
                pattern => operands.push(pattern),
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

    pub fn many0(pattern: Self) -> Self {
        match pattern {
            Self::Empty | Self::NotAllowed => Self::Empty,
            Self::Many0(pattern) | Self::Many1(pattern) | Self::Optional(pattern) => {
                Self::Many0(pattern)
            }
            pattern => Self::Many0(pattern.into()),
        }
    }

    pub fn many1(pattern: Self) -> Self {
        match pattern {
            Self::Empty => Self::Empty,
            Self::NotAllowed => Self::NotAllowed,
            Self::Many0(pattern) | Self::Optional(pattern) => Self::Many0(pattern),
            Self::Many1(pattern) => Self::Many1(pattern),
            pattern => Self::Many1(pattern.into()),
        }
    }

    pub fn optional(pattern: Self) -> Self {
        match pattern {
            Self::Empty | Self::NotAllowed => Self::Empty,
            Self::Many0(pattern) | Self::Many1(pattern) => Self::Many0(pattern),
            Self::Optional(pattern) => Self::Optional(pattern),
            pattern => Self::Optional(pattern.into()),
        }
    }
}

pub fn class_names(name_class: &NameClass) -> BTreeSet<String> {
    match name_class {
        NameClass::Name(name) => [identifier_string(&name.local)].into(),
        NameClass::Choice(classes) => classes.iter().flat_map(class_names).collect(),
        // TODO Support wildcard name classes (e.g. custom elements).
        NameClass::AnyName | NameClass::Except { .. } | NameClass::NamespaceName(_) => {
            Default::default()
        }
    }
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

fn identifier_string(identifier: &Identifier) -> String {
    identifier
        .sub_components
        .iter()
        .fold(identifier.component.clone(), |string, component| {
            string + "." + component
        })
}

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

            for operand in patterns {
                let operand_variants = split_pattern(operand)?;
                variants = variants
                    .iter()
                    .flat_map(|(attribute, content)| {
                        operand_variants
                            .iter()
                            .map(|(operand_attribute, operand_content)| {
                                (
                                    ResolvedPattern::interleave([
                                        attribute.clone(),
                                        operand_attribute.clone(),
                                    ]),
                                    if interleaved {
                                        ResolvedPattern::interleave([
                                            content.clone(),
                                            operand_content.clone(),
                                        ])
                                    } else {
                                        ResolvedPattern::group([
                                            content.clone(),
                                            operand_content.clone(),
                                        ])
                                    },
                                )
                            })
                            .collect::<Vec<_>>()
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

#[cfg(test)]
mod tests {
    use super::*;

    fn attribute(name: &str) -> ResolvedPattern {
        ResolvedPattern::Attribute([name.into()].into())
    }

    fn element(name: &str) -> ResolvedPattern {
        ResolvedPattern::Element([name.into()].into())
    }

    mod split_pattern {
        use super::*;
        use pretty_assertions::assert_eq;

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
                split_pattern(&ResolvedPattern::choice([attribute("foo"), element("bar")]))
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
}
