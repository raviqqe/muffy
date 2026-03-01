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
use std::{collections::HashMap, fs::read_to_string, path::Path};

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
    let schema_directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/schema/html5");
    let mut definitions = HashMap::new();

    load_schema(&schema_directory.join("html5.rnc"), &mut definitions)?;

    let mut element_groups = BTreeMap::<String, Vec<(String, Pattern)>>::new();

    for (name, pattern) in &definitions {
        let Pattern::Element { name_class, .. } = pattern else {
            continue;
        };
        let Some(element_name) = get_name(name_class) else {
            continue;
        };

        element_groups
            .entry(element_name)
            .or_default()
            .push((name.clone(), pattern.clone()));
    }

    let mut element_validators = vec![];
    let mut functions = vec![];

    for (element_name, defs) in element_groups {
        let validator_name =
            quote::format_ident!("validate_{}_element", element_name.replace('-', "_"));
        let element_name_str = element_name.clone();

        element_validators.push(quote! {
            #element_name_str => #validator_name(element),
        });

        let mut allowed_attributes = Vec::new();
        let mut allowed_children = Vec::new();

        for (_name, pattern) in defs {
            let Pattern::Element { pattern, .. } = pattern else {
                continue;
            };

            allowed_attributes.extend(collect_attributes(&pattern, &definitions));
            allowed_children.extend(collect_children(&pattern, &definitions));
        }

        allowed_attributes.sort();
        allowed_attributes.dedup();
        allowed_children.sort();
        allowed_children.dedup();

        let attribute_checks = allowed_attributes.iter().map(|attr| {
            quote! { #attr }
        });

        let child_checks = allowed_children.iter().map(|child| {
            quote! { #child }
        });

        functions.push(quote! {
            fn #validator_name(element: &Element) -> Result<(), ValidationError> {
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
                #(#element_validators)*
                _ => Err(ValidationError::InvalidElement(element.name().to_string())),
            }
        }

        #(#functions)*
    }
    .into())
}

fn load_schema(path: &Path, definitions: &mut HashMap<String, Pattern>) -> Result<(), MacroError> {
    let schema = parse_schema(&read_to_string(path)?)?;

    // We do not use the declarations.

    match schema.body {
        SchemaBody::Grammar(grammar) => {
            load_grammar(
                &grammar,
                path.parent().ok_or(MacroError::NoParentDirectory)?,
                definitions,
            )?;
        }
        SchemaBody::Pattern(_) => {
            return Err(MacroError::RncSyntax("top-level pattern").into());
        }
    }

    Ok(())
}

fn load_grammar(
    grammar: &Grammar,
    directory: &Path,
    definitions: &mut HashMap<String, Pattern>,
) -> Result<(), MacroError> {
    for content in &grammar.contents {
        match content {
            GrammarContent::Definition(def) => {
                let name = format_identifier(&def.name);
                let pattern = def.pattern.clone();

                if let Some(combine) = def.combine {
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
                // In a real implementation, we should handle overrides in include.grammar
                load_schema(&include_path, definitions)?;
            }
            GrammarContent::Div(grammar) => {
                load_grammar(grammar, directory, definitions)?;
            }
            _ => {}
        }
    }

    Ok(())
}

fn format_identifier(id: &Identifier) -> String {
    let mut string = id.component.clone();

    for component in &id.sub_components {
        string.push('.');
        string.push_str(component);
    }

    string
}

fn get_name(name_class: &NameClass) -> Option<String> {
    match name_class {
        NameClass::Name(name) => Some(name.local.component.clone()),
        NameClass::Choice(choices) => choices.iter().find_map(get_name),
        _ => None,
    }
}

fn collect_attributes(pattern: &Pattern, definitions: &HashMap<String, Pattern>) -> Vec<String> {
    let mut attributes = Vec::new();
    collect_attributes_recursive(pattern, definitions, &mut attributes, &mut Vec::new());
    attributes.sort();
    attributes.dedup();
    attributes
}

fn collect_attributes_recursive(
    pattern: &Pattern,
    definitions: &HashMap<String, Pattern>,
    attributes: &mut Vec<String>,
    visited: &mut Vec<String>,
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
            let def_name = format_identifier(&name.local);
            if !visited.contains(&def_name) {
                visited.push(def_name.clone());
                if let Some(p) = definitions.get(&def_name) {
                    collect_attributes_recursive(p, definitions, attributes, visited);
                }
            }
        }
        _ => {}
    }
}

fn collect_children(pattern: &Pattern, definitions: &HashMap<String, Pattern>) -> Vec<String> {
    let mut children = vec![];

    collect_children_recursive(pattern, definitions, &mut children, &mut Vec::new());

    children.sort();
    children.dedup();
    children
}

fn collect_children_recursive(
    pattern: &Pattern,
    definitions: &HashMap<String, Pattern>,
    children: &mut Vec<String>,
    visited: &mut Vec<String>,
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
            let name = format_identifier(&name.local);

            if !visited.contains(&name) {
                visited.push(name.clone());

                if let Some(p) = definitions.get(&name) {
                    collect_children_recursive(p, definitions, children, visited);
                }
            }
        }
        _ => {}
    }
}
