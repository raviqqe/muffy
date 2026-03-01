//! Macros for document validation.

use muffy_rnc::{
    Combine, Declaration, GrammarContent, Identifier, NameClass, Pattern, SchemaBody,
    parse_schema,
};
use proc_macro::TokenStream;
use quote::quote;
use std::{collections::HashMap, fs::read_to_string, path::Path};

/// Generates HTML validation functions.
#[proc_macro]
pub fn html(_input: TokenStream) -> TokenStream {
    let schema_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/schema/html5");
    let mut definitions = HashMap::new();

    load_schema(&schema_dir.join("html5.rnc"), &mut definitions);

    let mut element_validators = Vec::new();

    for (name, pattern) in &definitions {
        if let Pattern::Element { name_class, .. } = pattern {
            if let Some(element_name) = get_name(name_class) {
                let validator_name = quote::format_ident!("validate_{}_element", name_to_id(name));
                let element_name_str = element_name.clone();

                // For simplicity in this prototype, we just check element names.
                // A full implementation would check attributes and children recursively.
                element_validators.push(quote! {
                    #element_name_str => #validator_name(element),
                });
            }
        }
    }

    // Generate the functions
    let mut functions = Vec::new();
    for (name, pattern) in &definitions {
        if let Pattern::Element {
            name_class,
            pattern,
        } = pattern
        {
            if let Some(_element_name) = get_name(name_class) {
                let validator_name = quote::format_ident!("validate_{}_element", name_to_id(name));
                
                let allowed_attributes = collect_attributes(pattern, &definitions);
                let allowed_children = collect_children(pattern, &definitions);

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
        }
    }

    quote! {
        /// Validates an element.
        pub fn validate_element(element: &Element) -> Result<(), ValidationError> {
            match element.name() {
                #(#element_validators)*
                _ => Err(ValidationError::InvalidElement(element.name().to_string())),
            }
        }

        #(#functions)*
    }
    .into()
}

fn load_schema(path: &Path, definitions: &mut HashMap<String, Pattern>) {
    let content = read_to_string(path).unwrap();
    let schema = parse_schema(&content).unwrap();

    // Handle declarations (optional for this prototype)
    for decl in &schema.declarations {
        match decl {
            Declaration::DefaultNamespace(_) => {}
            Declaration::Namespace(_) => {}
            Declaration::Datatypes(_) => {}
        }
    }

    match schema.body {
        SchemaBody::Grammar(grammar) => {
            load_grammar(&grammar, path.parent().unwrap(), definitions);
        }
        SchemaBody::Pattern(_) => {
            // Root pattern, handle if needed
        }
    }
}

fn load_grammar(grammar: &muffy_rnc::Grammar, dir: &Path, definitions: &mut HashMap<String, Pattern>) {
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
                                let old = std::mem::replace(existing, Pattern::Choice(vec![]));
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
                                let old = std::mem::replace(existing, Pattern::Interleave(vec![]));
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
                let include_path = dir.join(&include.uri);
                // In a real implementation, we should handle overrides in include.grammar
                load_schema(&include_path, definitions);
            }
            GrammarContent::Div(grammar) => {
                load_grammar(grammar, dir, definitions);
            }
            _ => {}
        }
    }
}

fn format_identifier(id: &Identifier) -> String {
    let mut s = id.component.clone();
    for sub in &id.sub_components {
        s.push('.');
        s.push_str(sub);
    }
    s
}

fn name_to_id(name: &str) -> String {
    name.replace('.', "_").replace('-', "_")
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
    let mut children = Vec::new();
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
            for p in patterns {
                collect_children_recursive(p, definitions, children, visited);
            }
        }
        Pattern::Optional(p) | Pattern::Many0(p) | Pattern::Many1(p) => {
            collect_children_recursive(p, definitions, children, visited);
        }
        Pattern::Name(name) => {
            let def_name = format_identifier(&name.local);
            if !visited.contains(&def_name) {
                visited.push(def_name.clone());
                if let Some(p) = definitions.get(&def_name) {
                    collect_children_recursive(p, definitions, children, visited);
                }
            }
        }
        _ => {}
    }
}
