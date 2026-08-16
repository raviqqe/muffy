use crate::literal::{Literal, generate_literal};
use alloc::collections::BTreeSet;
use proc_macro2::TokenStream;
use quote::quote;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Value {
    Any,
    LiteralSet(BTreeSet<Literal>),
    TokenList(BTreeSet<Literal>),
}

impl Value {
    pub fn merge(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::LiteralSet(literals), Self::LiteralSet(others)) => {
                Self::LiteralSet(literals.union(others).cloned().collect())
            }
            (Self::TokenList(literals), Self::TokenList(others)) => {
                Self::TokenList(literals.union(others).cloned().collect())
            }
            _ => Self::Any,
        }
    }

    pub fn into_token_list(self) -> Self {
        match self {
            Self::LiteralSet(literals) => Self::TokenList(literals),
            value => value,
        }
    }
}

pub fn generate_value(value: &Value) -> TokenStream {
    match value {
        Value::Any => quote!(Value::Any),
        Value::LiteralSet(literals) => {
            let literals = generate_literals(literals);

            quote!(Value::LiteralSet(&[#(#literals),*]))
        }
        Value::TokenList(literals) => {
            let literals = generate_literals(literals);

            quote!(Value::TokenList(&[#(#literals),*]))
        }
    }
}

fn generate_literals(literals: &BTreeSet<Literal>) -> impl Iterator<Item = TokenStream> {
    literals.iter().map(generate_literal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

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
        fn merge_token_lists() {
            assert_eq!(
                Value::TokenList([Literal::Exact("foo".into())].into())
                    .merge(&Value::TokenList([Literal::Exact("bar".into())].into())),
                Value::TokenList(
                    [Literal::Exact("bar".into()), Literal::Exact("foo".into())].into()
                )
            );
        }

        #[test]
        fn merge_token_list_with_literals() {
            assert_eq!(
                Value::TokenList([Literal::Exact("foo".into())].into())
                    .merge(&Value::LiteralSet([Literal::Exact("bar".into())].into())),
                Value::Any
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
    fn generate_literal_set() {
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
    fn generate_token_list() {
        assert_eq!(
            generate_value(&Value::TokenList([Literal::Exact("foo".into())].into())).to_string(),
            quote!(Value::TokenList(&[Literal::Exact("foo")])).to_string()
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
