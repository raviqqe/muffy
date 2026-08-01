use crate::{error::MacroError, pattern::ResolvedPattern};
use alloc::collections::BTreeSet;
use proc_macro2::TokenStream;
use quote::quote;

const TEXT_TOKEN: &str = "#text";

pub fn generate_content(pattern: &ResolvedPattern) -> Result<TokenStream, MacroError> {
    Ok(match pattern {
        ResolvedPattern::Attribute(_) => {
            return Err(MacroError::RncPattern("attribute in content pattern"));
        }
        ResolvedPattern::NotAllowed => {
            return Err(MacroError::RncPattern("not-allowed content pattern"));
        }
        ResolvedPattern::Choice(patterns) => {
            let patterns = generate_contents(patterns)?;

            quote!(Content::Choice(&[#(#patterns),*]))
        }
        ResolvedPattern::Element(names) => {
            let names = names.iter().map(|name| quote!(#name));

            quote!(Content::Element(&[#(#names),*]))
        }
        ResolvedPattern::Empty => quote!(Content::Empty),
        ResolvedPattern::Group(patterns) => {
            let patterns = generate_contents(patterns)?;

            quote!(Content::Group(&[#(#patterns),*]))
        }
        ResolvedPattern::Interleave(patterns) => {
            let patterns = generate_contents(patterns)?;

            quote!(Content::Interleave(&[#(#patterns),*]))
        }
        ResolvedPattern::Many0(pattern) => {
            let pattern = generate_content(pattern)?;

            quote!(Content::Many0(&#pattern))
        }
        ResolvedPattern::Many1(pattern) => {
            let pattern = generate_content(pattern)?;

            quote!(Content::Many1(&#pattern))
        }
        ResolvedPattern::Optional(pattern) => {
            let pattern = generate_content(pattern)?;

            quote!(Content::Optional(&#pattern))
        }
        ResolvedPattern::Text => quote!(Content::Text),
    })
}

fn generate_contents(patterns: &[ResolvedPattern]) -> Result<Vec<TokenStream>, MacroError> {
    patterns.iter().map(generate_content).collect()
}

pub fn children(pattern: &ResolvedPattern) -> BTreeSet<String> {
    let mut children = element_names(pattern);

    if contains_text(pattern) {
        children.insert(TEXT_TOKEN.into());
    }

    children
}

fn contains_text(pattern: &ResolvedPattern) -> bool {
    match pattern {
        ResolvedPattern::Text => true,
        ResolvedPattern::Attribute(_)
        | ResolvedPattern::Element(_)
        | ResolvedPattern::Empty
        | ResolvedPattern::NotAllowed => false,
        ResolvedPattern::Choice(patterns)
        | ResolvedPattern::Group(patterns)
        | ResolvedPattern::Interleave(patterns) => patterns.iter().any(contains_text),
        ResolvedPattern::Many0(pattern)
        | ResolvedPattern::Many1(pattern)
        | ResolvedPattern::Optional(pattern) => contains_text(pattern),
    }
}

fn element_names(pattern: &ResolvedPattern) -> BTreeSet<String> {
    match pattern {
        ResolvedPattern::Element(names) => names.clone(),
        ResolvedPattern::Choice(patterns)
        | ResolvedPattern::Group(patterns)
        | ResolvedPattern::Interleave(patterns) => {
            patterns.iter().flat_map(element_names).collect()
        }
        ResolvedPattern::Many0(pattern)
        | ResolvedPattern::Many1(pattern)
        | ResolvedPattern::Optional(pattern) => element_names(pattern),
        ResolvedPattern::Attribute(_)
        | ResolvedPattern::Empty
        | ResolvedPattern::NotAllowed
        | ResolvedPattern::Text => Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn element(name: &str) -> ResolvedPattern {
        ResolvedPattern::Element([name.into()].into())
    }

    #[test]
    fn compile_ordered_group() {
        assert_eq!(
            generate_content(&ResolvedPattern::group([element("foo"), element("bar")]))
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
            generate_content(&ResolvedPattern::many0(element("foo")))
                .unwrap()
                .to_string(),
            quote!(Content::Many0(&Content::Element(&["foo"]))).to_string()
        );
    }

    #[test]
    fn collect_children_with_text() {
        assert_eq!(
            children(&ResolvedPattern::many0(ResolvedPattern::choice([
                ResolvedPattern::Text,
                element("foo"),
            ]))),
            ["#text".into(), "foo".into()].into()
        );
    }

    #[test]
    fn fail_on_attribute() {
        assert!(matches!(
            generate_content(&ResolvedPattern::Attribute(["foo".into()].into())),
            Err(MacroError::RncPattern(_))
        ));
    }
}
