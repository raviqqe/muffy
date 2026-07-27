use crate::{error::MacroError, pattern::CompiledPattern};
use alloc::collections::BTreeSet;

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct AttributeTerm {
    pub required: BTreeSet<String>,
    pub optional: BTreeSet<String>,
}

pub fn compile_attribute_terms(
    pattern: &CompiledPattern,
) -> Result<Vec<AttributeTerm>, MacroError> {
    let mut terms = compile(pattern)?;

    terms.sort();
    terms.dedup();

    Ok(terms)
}

fn compile(pattern: &CompiledPattern) -> Result<Vec<AttributeTerm>, MacroError> {
    Ok(match pattern {
        CompiledPattern::Empty => vec![Default::default()],
        CompiledPattern::NotAllowed => vec![],
        CompiledPattern::Attribute(names) => choice_terms(names),
        CompiledPattern::Choice(patterns) => patterns
            .iter()
            .map(compile)
            .collect::<Result<Vec<_>, _>>()?
            .concat(),
        CompiledPattern::Group(patterns) | CompiledPattern::Interleave(patterns) => {
            let mut terms = vec![AttributeTerm::default()];

            for operand in patterns {
                let operand_terms = compile(operand)?;
                terms = terms
                    .iter()
                    .flat_map(|term| {
                        operand_terms.iter().map(|operand_term| AttributeTerm {
                            required: term
                                .required
                                .iter()
                                .chain(&operand_term.required)
                                .cloned()
                                .collect(),
                            optional: term
                                .optional
                                .iter()
                                .chain(&operand_term.optional)
                                .cloned()
                                .collect(),
                        })
                    })
                    .collect();
            }

            terms
        }
        CompiledPattern::Optional(pattern) | CompiledPattern::Many0(pattern) => {
            optional_terms(compile(pattern)?)
        }
        CompiledPattern::Many1(pattern) => compile(pattern)?,
        CompiledPattern::Element(_) | CompiledPattern::Text => {
            return Err(MacroError::RncPattern("content in attribute pattern"));
        }
    })
}

fn optional_terms(mut terms: Vec<AttributeTerm>) -> Vec<AttributeTerm> {
    terms.sort();
    terms.dedup();

    let names = terms
        .iter()
        .flat_map(|term| term.required.iter().chain(&term.optional))
        .cloned()
        .collect::<BTreeSet<_>>();

    // Optional at-least-one-of terms accept every combination of the names,
    // which a single fully optional term encodes without any term blowup
    // through products of optional attributes.
    if terms == choice_terms(&names) {
        vec![AttributeTerm {
            required: Default::default(),
            optional: names,
        }]
    } else if terms.iter().any(|term| term.required.is_empty()) {
        terms
    } else {
        [AttributeTerm::default()]
            .into_iter()
            .chain(terms)
            .collect()
    }
}

fn choice_terms(names: &BTreeSet<String>) -> Vec<AttributeTerm> {
    names
        .iter()
        .map(|name| AttributeTerm {
            required: [name.clone()].into(),
            optional: names
                .iter()
                .filter(|other| *other != name)
                .cloned()
                .collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn attribute(name: &str) -> CompiledPattern {
        CompiledPattern::Attribute([name.into()].into())
    }

    fn term(required: &[&str], optional: &[&str]) -> AttributeTerm {
        AttributeTerm {
            required: required.iter().copied().map(Into::into).collect(),
            optional: optional.iter().copied().map(Into::into).collect(),
        }
    }

    #[test]
    fn compile_empty() {
        assert_eq!(
            compile_attribute_terms(&CompiledPattern::Empty).unwrap(),
            vec![AttributeTerm::default()]
        );
    }

    #[test]
    fn compile_not_allowed() {
        assert_eq!(
            compile_attribute_terms(&CompiledPattern::NotAllowed).unwrap(),
            vec![]
        );
    }

    #[test]
    fn compile_required_attribute() {
        assert_eq!(
            compile_attribute_terms(&attribute("foo")).unwrap(),
            vec![term(&["foo"], &[])]
        );
    }

    #[test]
    fn compile_optional_attribute() {
        assert_eq!(
            compile_attribute_terms(&CompiledPattern::optional(attribute("foo"))).unwrap(),
            vec![term(&[], &["foo"])]
        );
    }

    #[test]
    fn compile_interleave_of_optional_attributes() {
        assert_eq!(
            compile_attribute_terms(&CompiledPattern::interleave([
                CompiledPattern::optional(attribute("foo")),
                CompiledPattern::optional(attribute("bar")),
            ]))
            .unwrap(),
            vec![term(&[], &["bar", "foo"])]
        );
    }

    #[test]
    fn compile_choice_of_attributes() {
        assert_eq!(
            compile_attribute_terms(&CompiledPattern::choice([
                attribute("foo"),
                attribute("bar")
            ]))
            .unwrap(),
            vec![term(&["bar"], &[]), term(&["foo"], &[])]
        );
    }

    #[test]
    fn compile_group_product() {
        assert_eq!(
            compile_attribute_terms(&CompiledPattern::group([
                CompiledPattern::choice([attribute("foo"), attribute("bar")]),
                attribute("baz"),
            ]))
            .unwrap(),
            vec![term(&["bar", "baz"], &[]), term(&["baz", "foo"], &[])]
        );
    }

    #[test]
    fn compile_exclusive_attribute_pair() {
        assert_eq!(
            compile_attribute_terms(&CompiledPattern::choice([
                CompiledPattern::group([
                    attribute("foo"),
                    CompiledPattern::optional(attribute("bar"))
                ]),
                CompiledPattern::group([
                    CompiledPattern::optional(attribute("foo")),
                    attribute("bar")
                ]),
            ]))
            .unwrap(),
            vec![term(&["bar"], &["foo"]), term(&["foo"], &["bar"])]
        );
    }

    #[test]
    fn compile_at_least_one_attribute() {
        assert_eq!(
            compile_attribute_terms(&CompiledPattern::many1(attribute("foo"))).unwrap(),
            vec![term(&["foo"], &[])]
        );
    }

    #[test]
    fn compile_alternative_attribute_names() {
        assert_eq!(
            compile_attribute_terms(&CompiledPattern::Attribute(
                ["foo".into(), "bar".into()].into()
            ))
            .unwrap(),
            vec![term(&["bar"], &["foo"]), term(&["foo"], &["bar"])]
        );
    }

    #[test]
    fn compile_optional_alternative_attribute_names() {
        assert_eq!(
            compile_attribute_terms(&CompiledPattern::optional(CompiledPattern::Attribute(
                ["foo".into(), "bar".into()].into()
            )))
            .unwrap(),
            vec![term(&[], &["bar", "foo"])]
        );
    }

    #[test]
    fn compile_optional_choice_of_attributes() {
        // Exactly one of the exclusive attributes may be present.
        assert_eq!(
            compile_attribute_terms(&CompiledPattern::optional(CompiledPattern::choice([
                attribute("foo"),
                attribute("bar")
            ])))
            .unwrap(),
            vec![term(&[], &[]), term(&["bar"], &[]), term(&["foo"], &[])]
        );
    }

    #[test]
    fn fail_on_element() {
        assert!(matches!(
            compile_attribute_terms(&CompiledPattern::Element(["foo".into()].into())),
            Err(MacroError::RncPattern(_))
        ));
    }
}
