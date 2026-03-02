//! Macros for document validation.

extern crate alloc;

mod error;

use self::error::MacroError;
use alloc::collections::BTreeMap;
use muffy_rnc::{
    Combine, Grammar, GrammarContent, Identifier, NameClass, Pattern, SchemaBody, parse_schema,
};
use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use std::{
    collections::{HashMap, HashSet},
    fs::read_to_string,
    path::Path,
};

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
    let mut definitions = HashMap::new();

    load_schema(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/schema/html5")
            .join("html5.rnc"),
        &mut definitions,
    )?;

    // element_name -> (allowed_attributes, allowed_children)
    let mut element_rules = BTreeMap::<String, (Vec<String>, Vec<String>)>::new();

    for pattern in definitions.values() {
        let Pattern::Element { name_class, .. } = pattern else {
            continue;
        };
        let Some(element_name) = get_name(name_class) else {
            continue;
        };

        let (allowed_attributes, allowed_children) = element_rules
            .entry(element_name)
            .or_insert_with(|| (Vec::new(), Vec::new()));

        if let Pattern::Element { pattern, .. } = pattern {
            allowed_attributes.extend(collect_attributes(pattern, &definitions));
            allowed_children.extend(collect_children(pattern, &definitions));
        }
    }

    let mut element_matches = vec![];

    for (element_name, (mut attributes, mut children)) in element_rules {
        attributes.sort();
        attributes.dedup();
        children.sort();
        children.dedup();

        let attribute_checks = attributes.iter().map(|attr| {
            quote! { #attr }
        });

        let child_checks = children.iter().map(|child| {
            quote! { #child }
        });

        element_matches.push(quote! {
            #element_name => {
                for (name, _) in element.attributes() {
                    match name {
                        #(#attribute_checks |)* "xmlns" => {}
                        _ => return Err(ValidationError::InvalidAttribute(name.to_string())),
                    }
                }

                for child in element.children() {
                    if let muffy_document::html::Node::Element(child_element) = child {
                        match child_element.name() {
                            #(#child_checks |)* "!--" => {} // Allow comments
                            _ => return Err(ValidationError::InvalidChild(child_element.name().to_string())),
                        }
                    }
                }

                Ok(())
            }
        });
    }

    Ok(quote! {
        /// Validates an element.
        pub fn validate_element(element: &Element) -> Result<(), ValidationError> {
            match element.name() {
                #(#element_matches)*
                _ => Err(ValidationError::InvalidElement(element.name().to_string())),
            }
        }
    }
    .into())
}

fn load_schema(
    path: &Path,
    definitions: &mut HashMap<Identifier, Pattern>,
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
        SchemaBody::Pattern(_) => {
            return Err(MacroError::RncSyntax("top-level pattern"));
        }
    }

    Ok(())
}

fn load_grammar(
    grammar: &Grammar,
    definitions: &mut HashMap<Identifier, Pattern>,
    directory: &Path,
) -> Result<(), MacroError> {
    for content in &grammar.contents {
        match content {
            GrammarContent::Definition(definition) => {
                let name = definition.name.clone();
                let pattern = definition.pattern.clone();

                if let Some(combine) = definition.combine {
                    let existing = definitions.entry(name).or_insert(Pattern::NotAllowed);

                    match combine {
                        Combine::Choice => {
                            if let Pattern::Choice(choices) = existing {
                                choices.push(pattern);
                            } else if matches!(existing, Pattern::NotAllowed) {
                                *existing = pattern;
                            } else {
                                let old = core::mem::replace(existing, Pattern::Choice(vec![]));
                                if let Pattern::Choice(choices) = existing {
                                    choices.push(old);
                                    choices.push(pattern);
                                }
                            }
                        }
                        Combine::Interleave => {
                            if let Pattern::Interleave(patterns) = existing {
                                patterns.push(pattern);
                            } else if matches!(existing, Pattern::NotAllowed) {
                                *existing = pattern;
                            } else {
                                let old = core::mem::replace(existing, Pattern::Interleave(vec![]));
                                if let Pattern::Interleave(patterns) = existing {
                                    patterns.push(old);
                                    patterns.push(pattern);
                                }
                            }
                        }
                    }
                } else {
                    definitions.insert(name, pattern);
                }
            }
            GrammarContent::Include(include) => {
                let include_path = directory.join(&include.uri);

                // TODO Handle overrides in include.grammar.
                load_schema(&include_path, definitions)?;
            }
            GrammarContent::Div(grammar) => {
                load_grammar(grammar, definitions, directory)?;
            }
            GrammarContent::Annotation(_) | GrammarContent::Start { .. } => {}
        }
    }

    Ok(())
}

fn get_name(name_class: &NameClass) -> Option<String> {
    match name_class {
        NameClass::Name(name) => Some(name.local.component.clone()),
        NameClass::Choice(choices) => choices.iter().find_map(get_name),
        _ => None,
    }
}

fn collect_attributes(
    pattern: &Pattern,
    definitions: &HashMap<Identifier, Pattern>,
) -> Vec<String> {
    let mut attributes = Vec::new();
    collect_attributes_recursive(pattern, definitions, &mut attributes, &mut HashSet::new());
    attributes.sort();
    attributes.dedup();
    attributes
}

fn collect_attributes_recursive(
    pattern: &Pattern,
    definitions: &HashMap<Identifier, Pattern>,
    attributes: &mut Vec<String>,
    visited: &mut HashSet<Identifier>,
) {
    match pattern {
        Pattern::Attribute { name_class, .. } => {
            if let Some(name) = get_name(name_class) {
                attributes.push(name);
            }
        }
        Pattern::Choice(patterns) | Pattern::Group(patterns) | Pattern::Interleave(patterns) => {
            for p in patterns {
                collect_attributes_recursive(p, definitions, attributes, visited);
            }
        }
        Pattern::Optional(p) | Pattern::Many0(p) | Pattern::Many1(p) => {
            collect_attributes_recursive(p, definitions, attributes, visited);
        }
        Pattern::Name(name) => {
            if !visited.contains(&name.local) {
                visited.insert(name.local.clone());
                if let Some(p) = definitions.get(&name.local) {
                    collect_attributes_recursive(p, definitions, attributes, visited);
                }
            }
        }
        _ => {}
    }
}

fn collect_children(pattern: &Pattern, definitions: &HashMap<Identifier, Pattern>) -> Vec<String> {
    let mut children = vec![];

    collect_children_recursive(pattern, definitions, &mut children, &mut HashSet::new());

    children.sort();
    children.dedup();
    children
}

fn collect_children_recursive(
    pattern: &Pattern,
    definitions: &HashMap<Identifier, Pattern>,
    children: &mut Vec<String>,
    visited: &mut HashSet<Identifier>,
) {
    match pattern {
        Pattern::Element { name_class, .. } => {
            if let Some(name) = get_name(name_class) {
                children.push(name);
            }
        }
        Pattern::Choice(patterns) | Pattern::Group(patterns) | Pattern::Interleave(patterns) => {
            for pattern in patterns {
                collect_children_recursive(pattern, definitions, children, visited);
            }
        }
        Pattern::Optional(pattern) | Pattern::Many0(pattern) | Pattern::Many1(pattern) => {
            collect_children_recursive(pattern, definitions, children, visited);
        }
        Pattern::Name(name) => {
            if !visited.contains(&name.local) {
                visited.insert(name.local.clone());

                if let Some(p) = definitions.get(&name.local) {
                    collect_children_recursive(p, definitions, children, visited);
                }
            }
        }
        _ => {}
    }
}
