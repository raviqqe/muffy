use crate::error::MacroError;
use alloc::collections::{BTreeMap, BTreeSet};
use muffy_rnc::{Identifier, NameClass, Pattern};

const VARIANT_LIMIT: usize = 64;

/// A pattern compiled for name-based matching with references resolved,
/// attribute values dropped, and not-allowed sub-patterns propagated.
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

        let pattern = if alternatives.len() == 1 {
            alternatives.pop_first().expect("alternative")
        } else if alternatives.is_empty() {
            Self::NotAllowed
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

    pub fn nullable(&self) -> bool {
        match self {
            Self::Empty | Self::Many0(_) | Self::Optional(_) | Self::Text => true,
            Self::Attribute(_) | Self::Element(_) | Self::NotAllowed => false,
            Self::Choice(patterns) => patterns.iter().any(Self::nullable),
            Self::Group(patterns) | Self::Interleave(patterns) => {
                patterns.iter().all(Self::nullable)
            }
            Self::Many1(pattern) => pattern.nullable(),
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

// HTML parsers match a prefixed schema name (e.g. `xml:lang`) against its bare
// local name while the literal prefixed spelling is also conforming, so an
// attribute matches both names.
fn attribute_class_names(name_class: &NameClass) -> BTreeSet<String> {
    match name_class {
        NameClass::Name(name) => {
            let local = identifier_string(&name.local);

            if let Some(prefix) = &name.prefix {
                [format!("{}:{local}", identifier_string(prefix)), local].into()
            } else {
                [local].into()
            }
        }
        NameClass::Choice(classes) => classes.iter().flat_map(attribute_class_names).collect(),
        // TODO Support wildcard name classes (e.g. arbitrary attributes of
        // embed elements).
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

pub fn compile_pattern(
    pattern: &Pattern,
    definitions: &BTreeMap<Identifier, Pattern>,
    cache: &mut BTreeMap<Identifier, CompiledPattern>,
) -> Result<CompiledPattern, MacroError> {
    compile_pattern_with_stack(pattern, definitions, cache, &mut vec![])
}

fn compile_pattern_with_stack(
    pattern: &Pattern,
    definitions: &BTreeMap<Identifier, Pattern>,
    cache: &mut BTreeMap<Identifier, CompiledPattern>,
    stack: &mut Vec<Identifier>,
) -> Result<CompiledPattern, MacroError> {
    Ok(match pattern {
        Pattern::Attribute { name_class, .. } => {
            let names = attribute_class_names(name_class);

            if names.is_empty() {
                CompiledPattern::NotAllowed
            } else {
                CompiledPattern::Attribute(names)
            }
        }
        Pattern::Element { name_class, .. } => {
            let names = class_names(name_class);

            if names.is_empty() {
                CompiledPattern::NotAllowed
            } else {
                CompiledPattern::Element(names)
            }
        }
        Pattern::Choice(patterns) => CompiledPattern::choice(
            patterns
                .iter()
                .map(|pattern| compile_pattern_with_stack(pattern, definitions, cache, stack))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        Pattern::Group(patterns) => CompiledPattern::group(
            patterns
                .iter()
                .map(|pattern| compile_pattern_with_stack(pattern, definitions, cache, stack))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        Pattern::Interleave(patterns) => CompiledPattern::interleave(
            patterns
                .iter()
                .map(|pattern| compile_pattern_with_stack(pattern, definitions, cache, stack))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        Pattern::Many0(pattern) => CompiledPattern::many0(compile_pattern_with_stack(
            pattern,
            definitions,
            cache,
            stack,
        )?),
        Pattern::Many1(pattern) => CompiledPattern::many1(compile_pattern_with_stack(
            pattern,
            definitions,
            cache,
            stack,
        )?),
        Pattern::Optional(pattern) => CompiledPattern::optional(compile_pattern_with_stack(
            pattern,
            definitions,
            cache,
            stack,
        )?),
        Pattern::Empty => CompiledPattern::Empty,
        Pattern::NotAllowed => CompiledPattern::NotAllowed,
        // TODO Validate texts and attribute values against data and value
        // patterns.
        Pattern::Text | Pattern::Data { .. } | Pattern::List(_) | Pattern::Value { .. } => {
            CompiledPattern::Text
        }
        Pattern::External(_) => return Err(MacroError::RncPattern("external")),
        Pattern::Grammar(_) => return Err(MacroError::RncPattern("grammar")),
        Pattern::Name(name) => {
            if let Some(compiled) = cache.get(&name.local) {
                compiled.clone()
            } else if stack.contains(&name.local) {
                return Err(MacroError::CircularReference(identifier_string(
                    &name.local,
                )));
            } else if let Some(definition) = definitions.get(&name.local) {
                stack.push(name.local.clone());
                let compiled = compile_pattern_with_stack(definition, definitions, cache, stack)?;
                stack.pop();
                cache.insert(name.local.clone(), compiled.clone());
                compiled
            } else {
                return Err(MacroError::UndefinedReference(identifier_string(
                    &name.local,
                )));
            }
        }
    })
}

/// Splits a compiled element pattern into alternatives of attribute and
/// content patterns whose interleaved union is equivalent to the original.
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

                if variants.len() > VARIANT_LIMIT {
                    return Err(MacroError::PatternLimit("element pattern alternatives"));
                }
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
                // TODO Require at least one attribute for one-or-more
                // repetitions.
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
    use muffy_rnc::{SchemaBody, parse_schema};
    use std::path::Path;

    fn attribute(name: &str) -> CompiledPattern {
        CompiledPattern::Attribute([name.into()].into())
    }

    fn element(name: &str) -> CompiledPattern {
        CompiledPattern::Element([name.into()].into())
    }

    fn compile(source: &str) -> Result<CompiledPattern, MacroError> {
        let SchemaBody::Grammar(grammar) = parse_schema(source).unwrap().body else {
            panic!("grammar expected");
        };
        let mut definitions = BTreeMap::new();

        crate::load_grammar(&grammar, &mut definitions, Path::new(".")).unwrap();

        compile_pattern(
            &definitions[&Identifier {
                component: "root".into(),
                sub_components: vec![],
            }],
            &definitions,
            &mut Default::default(),
        )
    }

    mod compile_pattern {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn compile_attribute() {
            assert_eq!(
                compile("root = attribute foo { text }").unwrap(),
                attribute("foo")
            );
        }

        #[test]
        fn compile_prefixed_attribute_names() {
            assert_eq!(
                compile("root = attribute xml:lang { text }").unwrap(),
                CompiledPattern::Attribute(["lang".into(), "xml:lang".into()].into())
            );
        }

        #[test]
        fn resolve_reference() {
            assert_eq!(
                compile("root = foo\nfoo = attribute bar { text }").unwrap(),
                attribute("bar")
            );
        }

        #[test]
        fn evaluate_empty_flag() {
            assert_eq!(
                compile("root = attribute foo { text } & gate\ngate = empty").unwrap(),
                attribute("foo")
            );
        }

        #[test]
        fn evaluate_not_allowed_flag() {
            assert_eq!(
                compile("root = attribute foo { text } & gate\ngate = notAllowed").unwrap(),
                CompiledPattern::NotAllowed
            );
        }

        #[test]
        fn drop_wildcard_attribute_repetition() {
            assert_eq!(
                compile("root = attribute * { text }*").unwrap(),
                CompiledPattern::Empty
            );
        }

        #[test]
        fn combine_choice_of_element_names() {
            assert_eq!(
                compile("root = element (foo | bar) { empty }").unwrap(),
                CompiledPattern::Element(["bar".into(), "foo".into()].into())
            );
        }

        #[test]
        fn fail_on_undefined_reference() {
            assert!(matches!(
                compile("root = foo"),
                Err(MacroError::UndefinedReference(_))
            ));
        }

        #[test]
        fn fail_on_circular_reference() {
            assert!(matches!(
                compile("root = foo\nfoo = (foo)"),
                Err(MacroError::CircularReference(_))
            ));
        }
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
        fn fail_on_too_many_alternatives() {
            assert!(matches!(
                split_pattern(&CompiledPattern::Interleave(
                    (0..7)
                        .map(|index| CompiledPattern::choice([
                            attribute(&format!("foo{index}")),
                            element(&format!("bar{index}")),
                        ]))
                        .collect()
                )),
                Err(MacroError::PatternLimit(_))
            ));
        }

        #[test]
        fn split_not_allowed_into_no_alternative() {
            assert_eq!(split_pattern(&CompiledPattern::NotAllowed).unwrap(), vec![]);
        }
    }
}
