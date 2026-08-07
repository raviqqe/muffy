use alloc::collections::BTreeSet;
use muffy_rnc::{Identifier, NameClass};

// TODO Distinguish names in different namespaces so that schemas of multiple
// languages (e.g. HTML, SVG, and MathML) can compose without collisions of
// equal local names, like the driver schemas in the Nu Html Checker expect.
pub fn class_names(name_class: &NameClass) -> BTreeSet<String> {
    match name_class {
        NameClass::Name(name) => {
            let local = identifier_string(&name.local);

            [if let Some(prefix) = &name.prefix {
                format!("{}:{local}", identifier_string(prefix))
            } else {
                local
            }]
            .into()
        }
        NameClass::AnyName => ["*".into()].into(),
        NameClass::NamespaceName(prefix) => [if let Some(prefix) = prefix {
            format!("{}:*", identifier_string(prefix))
        } else {
            "*".into()
        }]
        .into(),
        NameClass::Choice(classes) => classes.iter().flat_map(class_names).collect(),
        // TODO Support name class exceptions (e.g. arbitrary attributes of
        // embed elements).
        NameClass::Except { .. } => Default::default(),
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
