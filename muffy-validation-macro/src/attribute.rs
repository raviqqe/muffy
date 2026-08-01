mod term;

pub use self::term::AttributeTerm;
use crate::{error::MacroError, pattern::CompiledPattern};
use alloc::collections::BTreeSet;

pub fn compile_attributes(pattern: &CompiledPattern) -> Result<Vec<AttributeTerm>, MacroError> {
    let mut terms = compile(pattern)?;

    terms.sort();
    terms.dedup();

    Ok(terms)
}

fn compile(pattern: &CompiledPattern) -> Result<Vec<AttributeTerm>, MacroError> {
    Ok(match pattern {
        CompiledPattern::Empty => vec![Default::default()],
        CompiledPattern::NotAllowed => vec![],
        CompiledPattern::Attribute(names) => name_choice(names),
        CompiledPattern::Choice(patterns) => patterns
            .iter()
            .map(compile)
            .collect::<Result<Vec<_>, _>>()?
            .concat(),
        CompiledPattern::Group(patterns) | CompiledPattern::Interleave(patterns) => {
            let mut terms = vec![AttributeTerm::default()];

            for pattern in patterns {
                let others = compile(pattern)?;

                terms = terms
                    .iter()
                    .flat_map(|term| others.iter().map(|other| term.merge(other)))
                    .collect();
            }

            terms
        }
        CompiledPattern::Optional(pattern) | CompiledPattern::Many0(pattern) => {
            optional(compile(pattern)?)
        }
        CompiledPattern::Many1(pattern) => compile(pattern)?,
        CompiledPattern::Element(_) | CompiledPattern::Text => {
            return Err(MacroError::RncPattern("content in attribute pattern"));
        }
    })
}

fn optional(mut terms: Vec<AttributeTerm>) -> Vec<AttributeTerm> {
    terms.sort();
    terms.dedup();

    let names = terms
        .iter()
        .flat_map(|term| term.required.iter().chain(&term.optional))
        .cloned()
        .collect::<BTreeSet<_>>();

    if terms == name_choice(&names) {
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

fn name_choice(names: &BTreeSet<String>) -> Vec<AttributeTerm> {
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
            compile_attributes(&CompiledPattern::Empty).unwrap(),
            vec![AttributeTerm::default()]
        );
    }

    #[test]
    fn compile_not_allowed() {
        assert_eq!(
            compile_attributes(&CompiledPattern::NotAllowed).unwrap(),
            vec![]
        );
    }

    #[test]
    fn compile_required_attribute() {
        assert_eq!(
            compile_attributes(&attribute("foo")).unwrap(),
            vec![term(&["foo"], &[])]
        );
    }

    #[test]
    fn compile_optional_attribute() {
        assert_eq!(
            compile_attributes(&CompiledPattern::optional(attribute("foo"))).unwrap(),
            vec![term(&[], &["foo"])]
        );
    }

    #[test]
    fn compile_interleave_of_optional_attributes() {
        assert_eq!(
            compile_attributes(&CompiledPattern::interleave([
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
            compile_attributes(&CompiledPattern::choice([
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
            compile_attributes(&CompiledPattern::group([
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
            compile_attributes(&CompiledPattern::choice([
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
            compile_attributes(&CompiledPattern::many1(attribute("foo"))).unwrap(),
            vec![term(&["foo"], &[])]
        );
    }

    #[test]
    fn compile_alternative_attribute_names() {
        assert_eq!(
            compile_attributes(&CompiledPattern::Attribute(
                ["foo".into(), "bar".into()].into()
            ))
            .unwrap(),
            vec![term(&["bar"], &["foo"]), term(&["foo"], &["bar"])]
        );
    }

    #[test]
    fn compile_optional_alternative_attribute_names() {
        assert_eq!(
            compile_attributes(&CompiledPattern::optional(CompiledPattern::Attribute(
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
            compile_attributes(&CompiledPattern::optional(CompiledPattern::choice([
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
            compile_attributes(&CompiledPattern::Element(["foo".into()].into())),
            Err(MacroError::RncPattern(_))
        ));
    }
}
