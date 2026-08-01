use crate::{error::MacroError, pattern::CompiledPattern};
use alloc::collections::BTreeSet;
use proc_macro2::TokenStream;
use quote::quote;

const TEXT_TOKEN: &str = "#text";

pub fn compile_content(pattern: &CompiledPattern) -> Result<TokenStream, MacroError> {
    Ok(match pattern {
        CompiledPattern::Attribute(_) => {
            return Err(MacroError::RncPattern("attribute in content pattern"));
        }
        CompiledPattern::NotAllowed => {
            return Err(MacroError::RncPattern("not-allowed content pattern"));
        }
        CompiledPattern::Choice(patterns) => {
            let patterns = compile_contents(patterns)?;

            quote!(Content::Choice(&[#(#patterns),*]))
        }
        CompiledPattern::Element(names) => {
            let names = names.iter().map(|name| quote!(#name));

            quote!(Content::Element(&[#(#names),*]))
        }
        CompiledPattern::Empty => quote!(Content::Empty),
        CompiledPattern::Group(patterns) => {
            let patterns = compile_contents(patterns)?;

            quote!(Content::Group(&[#(#patterns),*]))
        }
        CompiledPattern::Interleave(patterns) => {
            let patterns = compile_contents(patterns)?;

            quote!(Content::Interleave(&[#(#patterns),*]))
        }
        CompiledPattern::Many0(pattern) => {
            let pattern = compile_content(pattern)?;

            quote!(Content::Many0(&#pattern))
        }
        CompiledPattern::Many1(pattern) => {
            let pattern = compile_content(pattern)?;

            quote!(Content::Many1(&#pattern))
        }
        CompiledPattern::Optional(pattern) => {
            let pattern = compile_content(pattern)?;

            quote!(Content::Optional(&#pattern))
        }
        CompiledPattern::Text => quote!(Content::Text),
    })
}

fn compile_contents(patterns: &[CompiledPattern]) -> Result<Vec<TokenStream>, MacroError> {
    patterns.iter().map(compile_content).collect()
}

pub fn children(pattern: &CompiledPattern) -> BTreeSet<String> {
    let mut children = element_names(pattern);

    if contains_text(pattern) {
        children.insert(TEXT_TOKEN.into());
    }

    children
}

fn contains_text(pattern: &CompiledPattern) -> bool {
    match pattern {
        CompiledPattern::Text => true,
        CompiledPattern::Attribute(_)
        | CompiledPattern::Element(_)
        | CompiledPattern::Empty
        | CompiledPattern::NotAllowed => false,
        CompiledPattern::Choice(patterns)
        | CompiledPattern::Group(patterns)
        | CompiledPattern::Interleave(patterns) => patterns.iter().any(contains_text),
        CompiledPattern::Many0(pattern)
        | CompiledPattern::Many1(pattern)
        | CompiledPattern::Optional(pattern) => contains_text(pattern),
    }
}

fn element_names(pattern: &CompiledPattern) -> BTreeSet<String> {
    match pattern {
        CompiledPattern::Element(names) => names.clone(),
        CompiledPattern::Choice(patterns)
        | CompiledPattern::Group(patterns)
        | CompiledPattern::Interleave(patterns) => {
            patterns.iter().flat_map(element_names).collect()
        }
        CompiledPattern::Many0(pattern)
        | CompiledPattern::Many1(pattern)
        | CompiledPattern::Optional(pattern) => element_names(pattern),
        CompiledPattern::Attribute(_)
        | CompiledPattern::Empty
        | CompiledPattern::NotAllowed
        | CompiledPattern::Text => Default::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn element(name: &str) -> CompiledPattern {
        CompiledPattern::Element([name.into()].into())
    }

    #[test]
    fn compile_ordered_group() {
        assert_eq!(
            compile_content(&CompiledPattern::group([element("foo"), element("bar")]))
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
            compile_content(&CompiledPattern::many0(element("foo")))
                .unwrap()
                .to_string(),
            quote!(Content::Many0(&Content::Element(&["foo"]))).to_string()
        );
    }

    #[test]
    fn collect_children_with_text() {
        assert_eq!(
            children(&CompiledPattern::many0(CompiledPattern::choice([
                CompiledPattern::Text,
                element("foo"),
            ]))),
            ["#text".into(), "foo".into()].into()
        );
    }

    #[test]
    fn fail_on_attribute() {
        assert!(matches!(
            compile_content(&CompiledPattern::Attribute(["foo".into()].into())),
            Err(MacroError::RncPattern(_))
        ));
    }
}
