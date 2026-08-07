use alloc::collections::BTreeSet;
use muffy_rnc::NameClass;

// TODO Distinguish names in different namespaces so that schemas of multiple
// languages (e.g. HTML, SVG, and MathML) can compose without collisions of
// equal local names, like the driver schemas in the Nu Html Checker expect.
pub fn class_names(name_class: &NameClass) -> BTreeSet<String> {
    match name_class {
        NameClass::Name(name) => {
            let local = name.local.to_string();

            [if let Some(prefix) = &name.prefix {
                format!("{prefix}:{local}")
            } else {
                local
            }]
            .into()
        }
        NameClass::AnyName => ["*".into()].into(),
        NameClass::NamespaceName(prefix) => [if let Some(prefix) = prefix {
            format!("{prefix}:*")
        } else {
            // Wildcards in the empty namespace match names without prefixes.
            ":*".into()
        }]
        .into(),
        NameClass::Choice(classes) => classes.iter().flat_map(class_names).collect(),
        // TODO Support name class exceptions (e.g. arbitrary attributes of
        // embed elements).
        NameClass::Except { .. } => Default::default(),
    }
}
