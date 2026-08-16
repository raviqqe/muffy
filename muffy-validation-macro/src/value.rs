use alloc::collections::BTreeSet;
use muffy_rnc::DatatypeName;
use proc_macro2::TokenStream;
use quote::quote;

// The datatype library of the Nu Html Checker whose `string` datatype matches
// strings case-insensitively.
// TODO Resolve datatype prefixes against datatypes declarations.
const WHATTF_DATATYPE_PREFIX: &str = "w";

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Value {
    Any,
    LiteralSet(BTreeSet<Literal>),
}

impl Value {
    pub fn merge(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::LiteralSet(literals), Self::LiteralSet(others)) => {
                Self::LiteralSet(literals.union(others).cloned().collect())
            }
            _ => Self::Any,
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Literal {
    CaseInsensitive(String),
    Exact(String),
    Token(String),
}

impl Literal {
    pub fn new(name: Option<&DatatypeName>, value: &str) -> Option<Self> {
        match name {
            None | Some(DatatypeName::Token) => Some(Self::Token(value.into())),
            Some(DatatypeName::String) => Some(Self::Exact(value.into())),
            Some(DatatypeName::Name(name)) => (name.prefix.as_ref().map(ToString::to_string)
                == Some(WHATTF_DATATYPE_PREFIX.into())
                && name.local.to_string() == "string")
                .then(|| Self::CaseInsensitive(value.into())),
        }
    }
}

pub fn generate_value(value: &Value) -> TokenStream {
    match value {
        Value::Any => quote!(Value::Any),
        Value::LiteralSet(literals) => {
            let literals = literals.iter().map(|literal| match literal {
                Literal::CaseInsensitive(value) => quote!(Literal::CaseInsensitive(#value)),
                Literal::Exact(value) => quote!(Literal::Exact(#value)),
                Literal::Token(value) => quote!(Literal::Token(#value)),
            });

            quote!(Value::LiteralSet(&[#(#literals),*]))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use muffy_rnc::{Identifier, Name};
    use pretty_assertions::assert_eq;

    fn datatype(prefix: &str, local: &str) -> DatatypeName {
        DatatypeName::Name(Name {
            prefix: Some(Identifier {
                component: prefix.into(),
                sub_components: vec![],
            }),
            local: Identifier {
                component: local.into(),
                sub_components: vec![],
            },
        })
    }

    mod literal {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn create_token_literal_without_datatype() {
            assert_eq!(
                Literal::new(None, "foo"),
                Some(Literal::Token("foo".into()))
            );
        }

        #[test]
        fn create_token_literal() {
            assert_eq!(
                Literal::new(Some(&DatatypeName::Token), "foo"),
                Some(Literal::Token("foo".into()))
            );
        }

        #[test]
        fn create_exact_literal() {
            assert_eq!(
                Literal::new(Some(&DatatypeName::String), "foo"),
                Some(Literal::Exact("foo".into()))
            );
        }

        #[test]
        fn create_case_insensitive_literal() {
            assert_eq!(
                Literal::new(Some(&datatype("w", "string")), "foo"),
                Some(Literal::CaseInsensitive("foo".into()))
            );
        }

        #[test]
        fn create_no_literal_of_unknown_datatype() {
            assert_eq!(Literal::new(Some(&datatype("w", "language")), "foo"), None);
            assert_eq!(Literal::new(Some(&datatype("xsd", "string")), "foo"), None);
        }
    }

    mod merge {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn merge_literals() {
            assert_eq!(
                Value::LiteralSet([Literal::Token("foo".into())].into())
                    .merge(&Value::LiteralSet([Literal::Token("bar".into())].into())),
                Value::LiteralSet(
                    [Literal::Token("bar".into()), Literal::Token("foo".into())].into()
                )
            );
        }

        #[test]
        fn merge_any_value() {
            assert_eq!(
                Value::LiteralSet([Literal::Token("foo".into())].into()).merge(&Value::Any),
                Value::Any
            );
            assert_eq!(
                Value::Any.merge(&Value::LiteralSet([Literal::Token("foo".into())].into())),
                Value::Any
            );
        }
    }

    #[test]
    fn generate_literals() {
        assert_eq!(
            generate_value(&Value::LiteralSet(
                [
                    Literal::CaseInsensitive("foo".into()),
                    Literal::Token("bar".into())
                ]
                .into()
            ))
            .to_string(),
            quote!(Value::LiteralSet(&[
                Literal::CaseInsensitive("foo"),
                Literal::Token("bar")
            ]))
            .to_string()
        );
    }

    #[test]
    fn generate_any_value() {
        assert_eq!(
            generate_value(&Value::Any).to_string(),
            quote!(Value::Any).to_string()
        );
    }
}
