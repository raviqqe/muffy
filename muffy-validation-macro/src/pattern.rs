mod compile;
mod resolve;
mod resolved_pattern;
mod split;

pub use self::{compile::Compiler, resolved_pattern::ResolvedPattern};
use alloc::collections::BTreeSet;
use muffy_rnc::{Identifier, NameClass};

pub fn class_names(name_class: &NameClass, prefix: bool) -> BTreeSet<String> {
    match name_class {
        NameClass::Name(name) => {
            let local = identifier_string(&name.local);

            // HTML parsers match a prefixed schema name (e.g. `xml:lang`)
            // against its bare local name while the literal prefixed spelling
            // is also conforming, so an attribute matches both names.
            if let (true, Some(prefix)) = (prefix, &name.prefix) {
                [format!("{}:{local}", identifier_string(prefix)), local].into()
            } else {
                [local].into()
            }
        }
        NameClass::Choice(classes) => classes
            .iter()
            .flat_map(|class| class_names(class, prefix))
            .collect(),
        // TODO Support wildcard name classes (e.g. custom elements and
        // arbitrary attributes of embed elements).
        NameClass::AnyName | NameClass::Except { .. } | NameClass::NamespaceName(_) => {
            Default::default()
        }
    }
}

fn identifier_string(identifier: &Identifier) -> String {
    identifier
        .sub_components
        .iter()
        .fold(identifier.component.clone(), |string, component| {
            string + "." + component
        })
}
