use crate::{error::MacroError, pattern::Pattern};
use alloc::collections::BTreeSet;
use proc_macro2::TokenStream;
use quote::quote;

const TEXT_TOKEN: &str = "#text";

pub fn generate_content(pattern: &Pattern) -> Result<TokenStream, MacroError> {
    Ok(match pattern {
        Pattern::Attribute(..) => {
            return Err(MacroError::RncPattern("attribute in content pattern"));
        }
        Pattern::NotAllowed => {
            return Err(MacroError::RncPattern("not-allowed content pattern"));
        }
        Pattern::Choice(patterns) => {
            let patterns = generate_contents(patterns)?;

            quote!(Content::Choice(&[#(#patterns),*]))
        }
        Pattern::Element(names) => {
            let names = names.iter().map(|name| quote!(#name));

            quote!(Content::Element(&[#(#names),*]))
        }
        Pattern::Empty => quote!(Content::Empty),
        Pattern::Group(patterns) => {
            let patterns = generate_contents(patterns)?;

            quote!(Content::Group(&[#(#patterns),*]))
        }
        Pattern::Interleave(patterns) => {
            let patterns = generate_contents(patterns)?;

            quote!(Content::Interleave(&[#(#patterns),*]))
        }
        Pattern::Many0(pattern) => {
            let pattern = generate_content(pattern)?;

            quote!(Content::Many0(&#pattern))
        }
        Pattern::Many1(pattern) => {
            let pattern = generate_content(pattern)?;

            quote!(Content::Many1(&#pattern))
        }
        Pattern::Optional(pattern) => {
            let pattern = generate_content(pattern)?;

            quote!(Content::Optional(&#pattern))
        }
        Pattern::Text => quote!(Content::Text),
    })
}

fn generate_contents(patterns: &[Pattern]) -> Result<Vec<TokenStream>, MacroError> {
    patterns.iter().map(generate_content).collect()
}

pub fn children(pattern: &Pattern) -> BTreeSet<String> {
    let mut children = element_names(pattern);

    if contains_text(pattern) {
        children.insert(TEXT_TOKEN.into());
    }

    children
}

fn contains_text(pattern: &Pattern) -> bool {
    match pattern {
        Pattern::Text => true,
        Pattern::Attribute(..) | Pattern::Element(_) | Pattern::Empty | Pattern::NotAllowed => {
            false
        }
        Pattern::Choice(patterns) | Pattern::Group(patterns) | Pattern::Interleave(patterns) => {
            patterns.iter().any(contains_text)
        }
        Pattern::Many0(pattern) | Pattern::Many1(pattern) | Pattern::Optional(pattern) => {
            contains_text(pattern)
        }
    }
}

fn element_names(pattern: &Pattern) -> BTreeSet<String> {
    match pattern {
        Pattern::Element(names) => names.clone(),
        Pattern::Choice(patterns) | Pattern::Group(patterns) | Pattern::Interleave(patterns) => {
            patterns.iter().flat_map(element_names).collect()
        }
        Pattern::Many0(pattern) | Pattern::Many1(pattern) | Pattern::Optional(pattern) => {
            element_names(pattern)
        }
        Pattern::Attribute(..) | Pattern::Empty | Pattern::NotAllowed | Pattern::Text => {
            Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Value;
    use pretty_assertions::assert_eq;

    fn element(name: &str) -> Pattern {
        Pattern::Element([name.into()].into())
    }

    #[test]
    fn compile_ordered_group() {
        assert_eq!(
            generate_content(&Pattern::group([element("foo"), element("bar")]))
                .unwrap()
                .to_string(),
            quote!(Content::Group(&[
                Content::Element(&["foo"]),
                Content::Element(&["bar"])
            ]))
            .to_string()
        );
    }

    #[test]
    fn compile_repetition() {
        assert_eq!(
            generate_content(&Pattern::many0(element("foo")))
                .unwrap()
                .to_string(),
            quote!(Content::Many0(&Content::Element(&["foo"]))).to_string()
        );
    }

    #[test]
    fn collect_children_with_text() {
        assert_eq!(
            children(&Pattern::many0(Pattern::choice([
                Pattern::Text,
                element("foo"),
            ]))),
            ["#text".into(), "foo".into()].into()
        );
    }

    #[test]
    fn fail_on_attribute() {
        assert!(matches!(
            generate_content(&Pattern::Attribute(["foo".into()].into(), Value::Any)),
            Err(MacroError::RncPattern(_))
        ));
    }
}
