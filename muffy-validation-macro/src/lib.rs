//! Macros for document validation.

extern crate alloc;

mod attribute;
mod compiler;
mod content;
mod definition;
mod error;
mod name;
mod pattern;

use self::{
    attribute::AttributeSet,
    compiler::Compiler,
    content::{children, generate_content},
    definition::load_definitions,
    error::MacroError,
    name::class_names,
    pattern::Pattern,
};
use alloc::collections::BTreeMap;
use itertools::Itertools;
use muffy_rnc::{NameClass, Pattern as RncPattern};
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote};

/// Generates HTML validation functions.
#[proc_macro]
pub fn html(_input: TokenStream) -> TokenStream {
    // TODO Include SVG and MathML schemas for foreign elements.
    generate_validation("html", &["html.rnc"]).unwrap_or_else(|error| {
        syn::Error::new(Span::call_site(), error)
            .to_compile_error()
            .into()
    })
}

/// Generates SVG validation functions.
#[proc_macro]
pub fn svg(_input: TokenStream) -> TokenStream {
    generate_validation("svg", &["svg.rnc"]).unwrap_or_else(|error| {
        syn::Error::new(Span::call_site(), error)
            .to_compile_error()
            .into()
    })
}

fn generate_validation(language: &str, files: &[&str]) -> Result<TokenStream, MacroError> {
    let definitions = load_definitions(files)?;
    let mut compiler = Compiler::new(&definitions);
    let mut element_rules = BTreeMap::<String, Vec<(Vec<AttributeSet>, Pattern)>>::new();

    for definition in definitions.values() {
        for (name_class, pattern) in collect_elements(definition) {
            let names = class_names(name_class, false);

            if names.is_empty() {
                continue;
            }

            for (attribute_sets, content_pattern) in compiler.compile(pattern)? {
                for name in &names {
                    let variants = element_rules.entry(name.clone()).or_default();
                    let variant = (attribute_sets.clone(), content_pattern.clone());

                    if !variants.contains(&variant) {
                        variants.push(variant);
                    }
                }
            }
        }
    }

    let mut attribute_set_indexes = BTreeMap::<Vec<AttributeSet>, usize>::new();
    let mut content_indexes = BTreeMap::<Pattern, usize>::new();
    let mut element_matches = vec![];

    for (name, variants) in &element_rules {
        let attributes = variants
            .iter()
            .flat_map(|(sets, _)| sets)
            .flat_map(|set| set.required.iter().chain(&set.optional))
            .unique()
            .sorted()
            .map(|name| quote!(#name));
        let children = variants
            .iter()
            .flat_map(|(_, content)| children(content))
            .unique()
            .sorted()
            .map(|name| quote!(#name));

        let variants = variants
            .iter()
            .map(|(sets, content)| {
                let index = attribute_set_indexes.len();
                let sets = format_ident!(
                    "ATTRIBUTE_SETS_{}",
                    *attribute_set_indexes.entry(sets.clone()).or_insert(index)
                );

                let index = content_indexes.len();
                let content = format_ident!(
                    "CONTENT_{}",
                    *content_indexes.entry(content.clone()).or_insert(index)
                );

                quote!(Variant { attributes: #sets, content: &#content })
            })
            .collect::<Vec<_>>();

        element_matches.push(quote! {
            #name => {
                const RULE: Rule = Rule {
                    attributes: &[#(#attributes),*],
                    children: &[#(#children),*],
                    variants: &[#(#variants),*],
                };

                validate_rule(element, ignored_attributes, ignored_elements, &RULE)
            }
        });
    }

    let attribute_set_definitions = sort_by_index(attribute_set_indexes).map(|(sets, index)| {
        let identifier = format_ident!("ATTRIBUTE_SETS_{index}");
        let sets = sets.iter().map(|set| {
            let required = set.required.iter().map(|name| quote!(#name));
            let optional = set.optional.iter().map(|name| quote!(#name));

            quote!(AttributeSet {
                required: &[#(#required),*],
                optional: &[#(#optional),*],
            })
        });

        quote!(const #identifier: &[AttributeSet] = &[#(#sets),*];)
    });
    let content_definitions = sort_by_index(content_indexes)
        .map(|(content, index)| {
            let identifier = format_ident!("CONTENT_{index}");
            let content = generate_content(&content)?;

            Ok(quote!(const #identifier: Content = #content;))
        })
        .collect::<Result<Vec<_>, MacroError>>()?;

    let function_name = format_ident!("validate_{language}_element");
    let documentation = format!("Validates an {} element.", language.to_uppercase());

    Ok(quote! {
        #[doc = #documentation]
        pub fn #function_name(
            element: &Element,
            ignored_attributes: &[::regex::Regex],
            ignored_elements: &[::regex::Regex],
        ) -> Result<(), MarkupError> {
            #(#attribute_set_definitions)*
            #(#content_definitions)*

            match element.name() {
                name if ignored_elements.iter().any(|pattern| pattern.is_match(name)) => Ok(()),
                #(#element_matches)*
                _ => Err(MarkupError::UnknownTag(element.name().to_string())),
            }
        }
    }
    .into())
}

fn sort_by_index<T>(indexes: BTreeMap<T, usize>) -> impl Iterator<Item = (T, usize)> {
    let mut entries = indexes.into_iter().collect::<Vec<_>>();

    entries.sort_by_key(|(_, index)| *index);

    entries.into_iter()
}

// TODO Skip element definitions gated by not-allowed flag conjuncts.
fn collect_elements(pattern: &RncPattern) -> Vec<(&NameClass, &RncPattern)> {
    match pattern {
        RncPattern::Element {
            name_class,
            pattern,
        } => vec![(name_class, pattern)],
        RncPattern::Choice(patterns)
        | RncPattern::Group(patterns)
        | RncPattern::Interleave(patterns) => patterns.iter().flat_map(collect_elements).collect(),
        RncPattern::Many0(pattern) | RncPattern::Many1(pattern) | RncPattern::Optional(pattern) => {
            collect_elements(pattern)
        }
        RncPattern::Attribute { .. }
        | RncPattern::Data { .. }
        | RncPattern::Empty
        | RncPattern::External(_)
        | RncPattern::Grammar(_)
        | RncPattern::List(_)
        | RncPattern::Name(_)
        | RncPattern::NotAllowed
        | RncPattern::Text
        | RncPattern::Value { .. } => vec![],
    }
}
