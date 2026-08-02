use alloc::collections::BTreeSet;
use muffy_rnc::{Identifier, NameClass};

// TODO Distinguish names in different namespaces so that schemas of multiple
// languages (e.g. HTML, SVG, and MathML) can compose without collisions of
// equal local names, like the driver schemas in the Nu Html Checker expect.
pub fn class_names(name_class: &NameClass, prefix: bool) -> BTreeSet<String> {
    match name_class {
        NameClass::Name(name) => {
            let local = identifier_string(&name.local);

            if prefix && let Some(prefix) = &name.prefix {
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

pub fn identifier_string(identifier: &Identifier) -> String {
    identifier
        .sub_components
        .iter()
        .fold(identifier.component.clone(), |string, component| {
            string + "." + component
        })
}
