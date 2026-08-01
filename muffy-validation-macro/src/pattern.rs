mod compile;

pub use self::compile::compile_pattern;
use crate::error::MacroError;
use alloc::collections::BTreeSet;
use muffy_rnc::{Identifier, NameClass};

// TODO Support attribute value schemas.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CompiledPattern {
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

impl CompiledPattern {
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

fn attribute_names(pattern: &CompiledPattern) -> BTreeSet<String> {
    match pattern {
        CompiledPattern::Attribute(names) => names.clone(),
        CompiledPattern::Choice(patterns)
        | CompiledPattern::Group(patterns)
        | CompiledPattern::Interleave(patterns) => {
            patterns.iter().flat_map(attribute_names).collect()
        }
        CompiledPattern::Many0(pattern)
        | CompiledPattern::Many1(pattern)
        | CompiledPattern::Optional(pattern) => attribute_names(pattern),
        CompiledPattern::Element(_)
        | CompiledPattern::Empty
        | CompiledPattern::NotAllowed
        | CompiledPattern::Text => Default::default(),
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
    pattern: &CompiledPattern,
) -> Result<Vec<(CompiledPattern, CompiledPattern)>, MacroError> {
    Ok(match pattern {
        CompiledPattern::Attribute(_) => vec![(pattern.clone(), CompiledPattern::Empty)],
        CompiledPattern::Element(_) | CompiledPattern::Text => {
            vec![(CompiledPattern::Empty, pattern.clone())]
        }
        CompiledPattern::Empty => vec![(CompiledPattern::Empty, CompiledPattern::Empty)],
        CompiledPattern::NotAllowed => vec![],
        CompiledPattern::Group(patterns) | CompiledPattern::Interleave(patterns) => {
            let interleaved = matches!(pattern, CompiledPattern::Interleave(_));
            let mut variants = vec![(CompiledPattern::Empty, CompiledPattern::Empty)];

            for operand in patterns {
                let operand_variants = split_pattern(operand)?;
                variants = variants
                    .iter()
                    .flat_map(|(attribute, content)| {
                        operand_variants
                            .iter()
                            .map(|(operand_attribute, operand_content)| {
                                (
                                    CompiledPattern::interleave([
                                        attribute.clone(),
                                        operand_attribute.clone(),
                                    ]),
                                    if interleaved {
                                        CompiledPattern::interleave([
                                            content.clone(),
                                            operand_content.clone(),
                                        ])
                                    } else {
                                        CompiledPattern::group([
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
        CompiledPattern::Choice(patterns) => {
            let variants = patterns
                .iter()
                .map(split_pattern)
                .collect::<Result<Vec<_>, _>>()?;

            if variants
                .iter()
                .flatten()
                .all(|(_, content)| *content == CompiledPattern::Empty)
            {
                vec![(
                    CompiledPattern::choice(
                        variants
                            .into_iter()
                            .flatten()
                            .map(|(attribute, _)| attribute),
                    ),
                    CompiledPattern::Empty,
                )]
            } else if variants
                .iter()
                .flatten()
                .all(|(attribute, _)| *attribute == CompiledPattern::Empty)
            {
                vec![(
                    CompiledPattern::Empty,
                    CompiledPattern::choice(
                        variants.into_iter().flatten().map(|(_, content)| content),
                    ),
                )]
            } else {
                variants.into_iter().flatten().collect()
            }
        }
        CompiledPattern::Optional(pattern) => {
            let variants = split_pattern(pattern)?;

            match variants.as_slice() {
                [(attribute, content)] if *content == CompiledPattern::Empty => {
                    vec![(
                        CompiledPattern::optional(attribute.clone()),
                        CompiledPattern::Empty,
                    )]
                }
                [(attribute, content)] if *attribute == CompiledPattern::Empty => {
                    vec![(
                        CompiledPattern::Empty,
                        CompiledPattern::optional(content.clone()),
                    )]
                }
                _ => [(CompiledPattern::Empty, CompiledPattern::Empty)]
                    .into_iter()
                    .chain(variants)
                    .collect(),
            }
        }
        CompiledPattern::Many0(operand) | CompiledPattern::Many1(operand) => {
            let at_least_once = matches!(pattern, CompiledPattern::Many1(_));
            let variants = split_pattern(operand)?;

            if variants
                .iter()
                .all(|(attribute, _)| *attribute == CompiledPattern::Empty)
            {
                let content =
                    CompiledPattern::choice(variants.into_iter().map(|(_, content)| content));

                vec![(
                    CompiledPattern::Empty,
                    if at_least_once {
                        CompiledPattern::many1(content)
                    } else {
                        CompiledPattern::many0(content)
                    },
                )]
            } else if variants
                .iter()
                .all(|(_, content)| *content == CompiledPattern::Empty)
            {
                // Iterations of a repetition match alternatives independently
                // while attribute names never repeat on an element, so a
                // repetition accepts any combination of the attribute names.
                //
                // TODO Require at least one attribute for one-or-more repetitions.
                vec![(
                    CompiledPattern::interleave(
                        variants
                            .iter()
                            .flat_map(|(attribute, _)| attribute_names(attribute))
                            .collect::<BTreeSet<_>>()
                            .into_iter()
                            .map(|name| {
                                CompiledPattern::optional(CompiledPattern::Attribute([name].into()))
                            }),
                    ),
                    CompiledPattern::Empty,
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

    fn attribute(name: &str) -> CompiledPattern {
        CompiledPattern::Attribute([name.into()].into())
    }

    fn element(name: &str) -> CompiledPattern {
        CompiledPattern::Element([name.into()].into())
    }

    mod split_pattern {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn split_attribute_and_element() {
            assert_eq!(
                split_pattern(&CompiledPattern::interleave([
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
                split_pattern(&CompiledPattern::choice([
                    attribute("foo"),
                    attribute("bar")
                ]))
                .unwrap(),
                vec![(
                    CompiledPattern::choice([attribute("foo"), attribute("bar")]),
                    CompiledPattern::Empty
                )]
            );
        }

        #[test]
        fn lift_mixed_choice_into_alternatives() {
            assert_eq!(
                split_pattern(&CompiledPattern::choice([attribute("foo"), element("bar")]))
                    .unwrap(),
                vec![
                    (attribute("foo"), CompiledPattern::Empty),
                    (CompiledPattern::Empty, element("bar")),
                ]
            );
        }

        #[test]
        fn split_optional_attribute() {
            assert_eq!(
                split_pattern(&CompiledPattern::optional(attribute("foo"))).unwrap(),
                vec![(
                    CompiledPattern::optional(attribute("foo")),
                    CompiledPattern::Empty
                )]
            );
        }

        #[test]
        fn split_element_repetition() {
            assert_eq!(
                split_pattern(&CompiledPattern::many0(element("foo"))).unwrap(),
                vec![(
                    CompiledPattern::Empty,
                    CompiledPattern::many0(element("foo"))
                )]
            );
        }

        #[test]
        fn split_attribute_choice_repetition_into_optional_attributes() {
            assert_eq!(
                split_pattern(&CompiledPattern::many1(CompiledPattern::choice([
                    attribute("foo"),
                    attribute("bar")
                ])))
                .unwrap(),
                vec![(
                    CompiledPattern::interleave([
                        CompiledPattern::optional(attribute("bar")),
                        CompiledPattern::optional(attribute("foo")),
                    ]),
                    CompiledPattern::Empty
                )]
            );
        }

        #[test]
        fn split_not_allowed_into_no_alternative() {
            assert_eq!(split_pattern(&CompiledPattern::NotAllowed).unwrap(), vec![]);
        }
    }
}
