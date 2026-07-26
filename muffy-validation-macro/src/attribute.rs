use crate::{error::MacroError, pattern::CompiledPattern};
use alloc::collections::BTreeSet;

const TERM_LIMIT: usize = 1024;

/// One alternative of attribute names an element accepts: all required names
/// must be present, and no name outside the required and optional ones may be.
#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct AttributeTerm {
    pub required: BTreeSet<String>,
    pub optional: BTreeSet<String>,
}

pub fn compile_attribute_terms(
    pattern: &CompiledPattern,
) -> Result<Vec<AttributeTerm>, MacroError> {
    let mut terms = compile_terms(pattern)?;

    terms.sort();
    terms.dedup();

    Ok(terms)
}

fn compile_terms(pattern: &CompiledPattern) -> Result<Vec<AttributeTerm>, MacroError> {
    Ok(match pattern {
        CompiledPattern::Empty => vec![AttributeTerm::default()],
        CompiledPattern::NotAllowed => vec![],
        CompiledPattern::Attribute(names) => names
            .iter()
            .map(|name| AttributeTerm {
                required: [name.clone()].into(),
                optional: Default::default(),
            })
            .collect(),
        CompiledPattern::Choice(patterns) => {
            let terms = patterns
                .iter()
                .map(compile_terms)
                .collect::<Result<Vec<_>, _>>()?
                .concat();

            if terms.len() > TERM_LIMIT {
                return Err(MacroError::PatternLimit("attribute alternatives"));
            }

            terms
        }
        CompiledPattern::Group(patterns) | CompiledPattern::Interleave(patterns) => {
            let mut terms = vec![AttributeTerm::default()];

            for operand in patterns {
                let operand_terms = compile_terms(operand)?;
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

                if terms.len() > TERM_LIMIT {
                    return Err(MacroError::PatternLimit("attribute alternatives"));
                }
            }

            terms
        }
        CompiledPattern::Optional(pattern) | CompiledPattern::Many0(pattern) => {
            optional_terms(compile_terms(pattern)?)
        }
        // Attributes never repeat on an element.
        CompiledPattern::Many1(pattern) => compile_terms(pattern)?,
        CompiledPattern::Element(_) | CompiledPattern::Text => {
            return Err(MacroError::RncPattern("content in attribute pattern"));
        }
    })
}

fn optional_terms(terms: Vec<AttributeTerm>) -> Vec<AttributeTerm> {
    if let [term] = terms.as_slice()
        && term.optional.is_empty()
        && term.required.len() == 1
    {
        vec![AttributeTerm {
            required: Default::default(),
            optional: term.required.clone(),
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
    fn fail_on_element() {
        assert!(matches!(
            compile_attribute_terms(&CompiledPattern::Element(["foo".into()].into())),
            Err(MacroError::RncPattern(_))
        ));
    }
}
