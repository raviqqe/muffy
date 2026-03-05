//! Macros for document validation.

extern crate alloc;

mod error;

use self::error::MacroError;
use alloc::collections::{BTreeMap, BTreeSet};
use core::mem::replace;
use muffy_rnc::{
    Combine, Grammar, GrammarContent, Identifier, NameClass, Pattern, SchemaBody, parse_schema,
};
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
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

    load_schema(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/schema/html5")
            .join("html5.rnc"),
        &mut definitions,
    )?;

    // element -> (attributes, children)
    let mut element_rules = BTreeMap::<String, (Vec<String>, Vec<String>)>::new();

    for pattern in definitions.values() {
        let Pattern::Element { name_class, .. } = pattern else {
            continue;
        };
        let Some(element) = get_name(name_class) else {
            continue;
        };

        if let Pattern::Element { pattern, .. } = pattern {
            let (attributes, children) = element_rules
                .entry(element)
                .or_insert_with(|| (vec![], vec![]));

            attributes.extend(collect_attributes(pattern, &definitions)?);
            children.extend(collect_children(pattern, &definitions)?);
        }
    }

    let mut element_matches = vec![];

    for (element, (mut attributes, mut children)) in element_rules {
        attributes.sort();
        attributes.dedup();
        children.sort();
        children.dedup();

        let attributes = attributes.iter().map(|attr| quote!(#attr));
        let children = children.iter().map(|child| quote!(#child));

        element_matches.push(quote! {
            #element => {
                let mut attributes = ::alloc::collections::BTreeMap::new();

                for (attribute_name, _) in element.attributes() {
                    match attribute_name {
                        #(#attributes |)* "_DUMMY_" => {}
                        _ => {
                            let attribute_name_string = attribute_name.to_string();
                            attributes.insert(
                                attribute_name_string.clone(),
                                AttributeError::Invalid,
                            );
                        }
                    }
                }

                let mut children = ::alloc::collections::BTreeMap::new();

                for child in element.children() {
                    if let muffy_document::html::Node::Element(child_element) = child {
                        let child_name = child_element.name();

                        match child_name {
                            #(#children |)* "_DUMMY_" => {}
                            _ => {
                                let child_name_string = child_name.to_string();
                                children.insert(
                                    child_name_string.clone(),
                                    ChildError::Invalid,
                                );
                            }
                        }
                    }
                }

                if attributes.is_empty() && children.is_empty() {
                    Ok(())
                } else {
                    Err(ValidationError::InvalidElementDetails {
                        attributes,
                        children,
                    })
                }
            }
        });
    }

    Ok(quote! {
        /// Validates an element.
        pub fn validate_element(element: &Element) -> Result<(), ValidationError> {
            match element.name() {
                #(#element_matches)*
                _ => Err(ValidationError::InvalidTag(element.name().to_string())),
            }
        }
    }
    .into())
}

fn load_schema(
    path: &Path,
    definitions: &mut BTreeMap<Identifier, Pattern>,
) -> Result<(), MacroError> {
    let schema = parse_schema(&read_to_string(path)?)?;

    // We do not use the declarations.

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

fn get_name(name_class: &NameClass) -> Option<String> {
    match name_class {
        NameClass::Name(name) => Some(name.local.component.clone()),
        NameClass::Choice(choices) => choices.iter().find_map(get_name),
        NameClass::AnyName | NameClass::Except { .. } | NameClass::NamespaceName(_) => None,
    }
}

fn collect_attributes(
    pattern: &Pattern,
    definitions: &BTreeMap<Identifier, Pattern>,
) -> Result<BTreeSet<String>, MacroError> {
    let mut attributes = Default::default();

    collect_nested_attributes(
        pattern,
        definitions,
        &mut attributes,
        &mut Default::default(),
    )?;

    Ok(attributes)
}

fn collect_nested_attributes<'a>(
    pattern: &'a Pattern,
    definitions: &'a BTreeMap<Identifier, Pattern>,
    attributes: &mut BTreeSet<String>,
    visited: &mut BTreeSet<&'a Identifier>,
) -> Result<(), MacroError> {
    match pattern {
        Pattern::Attribute { name_class, .. } => {
            if let Some(name) = get_name(name_class) {
                attributes.insert(name);
            }
        }
        Pattern::Name(name) => {
            if !visited.contains(&name.local) {
                visited.insert(&name.local);

                if let Some(pattern) = definitions.get(&name.local) {
                    collect_nested_attributes(pattern, definitions, attributes, visited)?;
                }
            }
        }
        Pattern::Choice(patterns) | Pattern::Group(patterns) | Pattern::Interleave(patterns) => {
            for pattern in patterns {
                collect_nested_attributes(pattern, definitions, attributes, visited)?;
            }
        }
        Pattern::Many0(pattern) | Pattern::Many1(pattern) | Pattern::Optional(pattern) => {
            collect_nested_attributes(pattern, definitions, attributes, visited)?;
        }
        Pattern::Data { .. } => return Err(MacroError::RncPattern("data")),
        Pattern::External(_) => return Err(MacroError::RncPattern("external")),
        Pattern::Grammar(_) => return Err(MacroError::RncPattern("grammar")),
        Pattern::List { .. } => return Err(MacroError::RncPattern("list")),
        Pattern::Value { .. } => return Err(MacroError::RncPattern("value")),
        Pattern::Empty | Pattern::Element { .. } | Pattern::NotAllowed | Pattern::Text => {}
    }

    Ok(())
}

fn collect_children(
    pattern: &Pattern,
    definitions: &BTreeMap<Identifier, Pattern>,
) -> Result<BTreeSet<String>, MacroError> {
    let mut children = Default::default();

    collect_nested_children(pattern, definitions, &mut children, &mut Default::default())?;

    Ok(children)
}

fn collect_nested_children<'a>(
    pattern: &'a Pattern,
    definitions: &'a BTreeMap<Identifier, Pattern>,
    children: &mut BTreeSet<String>,
    visited: &mut BTreeSet<&'a Identifier>,
) -> Result<(), MacroError> {
    match pattern {
        Pattern::Element { name_class, .. } => {
            if let Some(name) = get_name(name_class) {
                children.insert(name);
            }
        }
        Pattern::Name(name) => {
            if !visited.contains(&name.local) {
                visited.insert(&name.local);

                if let Some(pattern) = definitions.get(&name.local) {
                    collect_nested_children(pattern, definitions, children, visited)?;
                }
            }
        }
        Pattern::Choice(patterns) | Pattern::Group(patterns) | Pattern::Interleave(patterns) => {
            for pattern in patterns {
                collect_nested_children(pattern, definitions, children, visited)?;
            }
        }
        Pattern::Many0(pattern) | Pattern::Many1(pattern) | Pattern::Optional(pattern) => {
            collect_nested_children(pattern, definitions, children, visited)?;
        }
        Pattern::Data { .. } => return Err(MacroError::RncPattern("data")),
        Pattern::External(_) => return Err(MacroError::RncPattern("external")),
        Pattern::Grammar(_) => return Err(MacroError::RncPattern("grammar")),
        Pattern::List { .. } => return Err(MacroError::RncPattern("list")),
        Pattern::Value { .. } => return Err(MacroError::RncPattern("value")),
        Pattern::Attribute { .. } | Pattern::Empty | Pattern::NotAllowed | Pattern::Text => {}
    }

    Ok(())
}
