//! Macros for document validation.

extern crate alloc;

mod attribute;
mod content;
mod error;
mod name;
mod pattern;

use self::{
    attribute::{AttributeSet, compile_attributes},
    content::{children, compile_content},
    error::MacroError,
    name::class_names,
    pattern::{Compiler, ResolvedPattern},
};
use alloc::collections::{BTreeMap, BTreeSet};
use core::mem::replace;
use muffy_rnc::{
    Combine, Grammar, GrammarContent, Identifier, NameClass, Pattern, SchemaBody, parse_schema,
};
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote};
use std::{fs::read_to_string, path::Path};

/// Generates HTML validation functions.
#[proc_macro]
pub fn html(_input: TokenStream) -> TokenStream {
    generate_html().unwrap_or_else(|error| {
        syn::Error::new(Span::call_site(), error)
            .to_compile_error()
            .into()
    })
}

fn generate_html() -> Result<TokenStream, MacroError> {
    let mut definitions = Default::default();

    // TODO Include SVG and MathML schemas.
    for file in ["html5.rnc", "rdfa.rnc"] {
        load_schema(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src")
                .join("schema")
                .join("html5")
                .join(file),
            &mut definitions,
        )?;
    }

    let mut compiler = Compiler::new(&definitions);
    let mut element_rules = BTreeMap::<String, Vec<(Vec<AttributeSet>, ResolvedPattern)>>::new();

    for definition in definitions.values() {
        for (name_class, pattern) in collect_elements(definition) {
            let names = class_names(name_class, false);

            if names.is_empty() {
                continue;
            }

            for (attribute_pattern, content_pattern) in compiler.compile(pattern)? {
                let sets = compile_attributes(&attribute_pattern)?;

                if sets.is_empty() || content_pattern == ResolvedPattern::NotAllowed {
                    continue;
                }

                for name in &names {
                    let variants = element_rules.entry(name.clone()).or_default();
                    let variant = (sets.clone(), content_pattern.clone());

                    if !variants.contains(&variant) {
                        variants.push(variant);
                    }
                }
            }
        }
    }

    let mut attribute_set_indexes = BTreeMap::<Vec<AttributeSet>, usize>::new();
    let mut content_indexes = BTreeMap::<ResolvedPattern, usize>::new();
    let mut element_matches = vec![];

    for (name, variants) in &element_rules {
        let attributes = variants
            .iter()
            .flat_map(|(sets, _)| sets)
            .flat_map(|set| set.required.iter().chain(&set.optional))
            .collect::<BTreeSet<_>>();
        let children = variants
            .iter()
            .flat_map(|(_, content)| children(content))
            .collect::<BTreeSet<_>>();

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
        let attributes = attributes.iter().map(|name| quote!(#name));
        let children = children.iter().map(|name| quote!(#name));

        element_matches.push(quote! {
            #name => {
                static RULE: Rule = Rule {
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

        quote!(static #identifier: &[AttributeSet] = &[#(#sets),*];)
    });
    let content_definitions = sort_by_index(content_indexes)
        .map(|(content, index)| {
            let identifier = format_ident!("CONTENT_{index}");
            let content = compile_content(&content)?;

            Ok(quote!(static #identifier: Content = #content;))
        })
        .collect::<Result<Vec<_>, MacroError>>()?;

    Ok(quote! {
        /// Validates an HTML element.
        pub fn validate_html_element(
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

fn load_schema(
    path: &Path,
    definitions: &mut BTreeMap<Identifier, Pattern>,
) -> Result<(), MacroError> {
    let schema = parse_schema(&read_to_string(path)?)?;

    // We do not use the declarations.
    // TODO Respect namespace declarations.

    match schema.body {
        SchemaBody::Grammar(grammar) => {
            load_grammar(
                &grammar,
                definitions,
                path.parent().ok_or(MacroError::NoParentDirectory)?,
            )?;
        }
        SchemaBody::Pattern(_) => return Err(MacroError::RncSyntax("top-level pattern")),
    }

    Ok(())
}

fn load_grammar(
    grammar: &Grammar,
    definitions: &mut BTreeMap<Identifier, Pattern>,
    directory: &Path,
) -> Result<(), MacroError> {
    for content in &grammar.contents {
        match content {
            GrammarContent::Definition(definition) => {
                let name = definition.name.clone();
                let pattern = definition.pattern.clone();

                if let Some(combine) = definition.combine {
                    combine_patterns(
                        definitions.entry(name).or_insert(Pattern::NotAllowed),
                        pattern,
                        combine,
                    );
                } else {
                    definitions.insert(name, pattern);
                }
            }
            GrammarContent::Div(grammar) => load_grammar(grammar, definitions, directory)?,
            GrammarContent::Include(include) => {
                let include_path = directory.join(&include.uri);

                load_schema(&include_path, definitions)?;

                if let Some(grammar) = &include.grammar {
                    load_grammar(grammar, definitions, directory)?;
                }
            }
            GrammarContent::Annotation(_) | GrammarContent::Start { .. } => {}
        }
    }

    Ok(())
}

fn combine_patterns(existing: &mut Pattern, new: Pattern, combine: Combine) {
    match combine {
        Combine::Choice => match existing {
            Pattern::Choice(choices) => choices.push(new),
            Pattern::NotAllowed => *existing = new,
            Pattern::Attribute { .. }
            | Pattern::Data { .. }
            | Pattern::Element { .. }
            | Pattern::Empty
            | Pattern::External(_)
            | Pattern::Grammar(_)
            | Pattern::Group(_)
            | Pattern::Interleave(_)
            | Pattern::List(_)
            | Pattern::Many0(_)
            | Pattern::Many1(_)
            | Pattern::Name(_)
            | Pattern::Optional(_)
            | Pattern::Text
            | Pattern::Value { .. } => {
                let old = replace(existing, Pattern::Choice(vec![]));

                if let Pattern::Choice(choices) = existing {
                    choices.push(old);
                    choices.push(new);
                }
            }
        },
        Combine::Interleave => match existing {
            Pattern::Interleave(patterns) => patterns.push(new),
            Pattern::NotAllowed => *existing = new,
            Pattern::Attribute { .. }
            | Pattern::Choice(_)
            | Pattern::Data { .. }
            | Pattern::Element { .. }
            | Pattern::Empty
            | Pattern::External(_)
            | Pattern::Grammar(_)
            | Pattern::Group(_)
            | Pattern::List(_)
            | Pattern::Many0(_)
            | Pattern::Many1(_)
            | Pattern::Name(_)
            | Pattern::Optional(_)
            | Pattern::Text
            | Pattern::Value { .. } => {
                let old = replace(existing, Pattern::Interleave(vec![]));

                if let Pattern::Interleave(patterns) = existing {
                    patterns.push(old);
                    patterns.push(new);
                }
            }
        },
    }
}

// TODO Skip element definitions gated by not-allowed flag conjuncts.
fn collect_elements(pattern: &Pattern) -> Vec<(&NameClass, &Pattern)> {
    match pattern {
        Pattern::Element {
            name_class,
            pattern,
        } => vec![(name_class, pattern)],
        Pattern::Choice(patterns) | Pattern::Group(patterns) | Pattern::Interleave(patterns) => {
            patterns.iter().flat_map(collect_elements).collect()
        }
        Pattern::Many0(pattern) | Pattern::Many1(pattern) | Pattern::Optional(pattern) => {
            collect_elements(pattern)
        }
        Pattern::Attribute { .. }
        | Pattern::Data { .. }
        | Pattern::Empty
        | Pattern::External(_)
        | Pattern::Grammar(_)
        | Pattern::List(_)
        | Pattern::Name(_)
        | Pattern::NotAllowed
        | Pattern::Text
        | Pattern::Value { .. } => vec![],
    }
}
