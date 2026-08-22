use muffy_rnc::DatatypeName;
use proc_macro2::TokenStream;
use quote::quote;

// TODO Resolve datatype prefixes against datatypes declarations.
const WHATTF_DATATYPE_PREFIX: &str = "w";

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Literal {
    CaseInsensitive(String),
    Exact(String),
    Pattern(String),
    Token(String),
}

impl Literal {
    // TODO Create literals of other datatypes like XSD strings and tokens.
    pub fn new(name: Option<&DatatypeName>, value: &str) -> Option<Self> {
        match name {
            None | Some(DatatypeName::Token) => Some(Self::Token(value.into())),
            Some(DatatypeName::String) => Some(Self::Exact(value.into())),
            // `w:string` in vnu matches strings case-insensitively.
            Some(DatatypeName::Name(name)) => (name.prefix.as_ref().map(ToString::to_string)
                == Some(WHATTF_DATATYPE_PREFIX.into())
                && name.local.to_string() == "string")
                .then(|| Self::CaseInsensitive(value.into())),
        }
    }
}

pub fn generate_literal(literal: &Literal) -> TokenStream {
    match literal {
        Literal::CaseInsensitive(value) => quote!(Literal::CaseInsensitive(#value)),
        Literal::Exact(value) => quote!(Literal::Exact(#value)),
        Literal::Pattern(value) => quote!(pattern!(#value)),
        Literal::Token(value) => quote!(Literal::Token(#value)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use muffy_rnc::{Identifier, Name};

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

    mod new {
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

    mod generate {
        use super::*;
        use pretty_assertions::assert_eq;

        #[test]
        fn generate_case_insensitive_literal() {
            assert_eq!(
                generate_literal(&Literal::CaseInsensitive("foo".into())).to_string(),
                quote!(Literal::CaseInsensitive("foo")).to_string()
            );
        }

        #[test]
        fn generate_exact_literal() {
            assert_eq!(
                generate_literal(&Literal::Exact("foo".into())).to_string(),
                quote!(Literal::Exact("foo")).to_string()
            );
        }

        #[test]
        fn generate_pattern_literal() {
            assert_eq!(
                generate_literal(&Literal::Pattern("a+".into())).to_string(),
                quote!(pattern!("a+")).to_string()
            );
        }

        #[test]
        fn generate_token_literal() {
            assert_eq!(
                generate_literal(&Literal::Token("foo".into())).to_string(),
                quote!(Literal::Token("foo")).to_string()
            );
        }
    }
}
