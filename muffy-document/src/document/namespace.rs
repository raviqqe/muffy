use html5ever::QualName;

// TODO Is it valid to handle all markup languages with a single driver schema?
const DEFAULT_NAMESPACES: &[&str] = &[
    "",
    "http://www.w3.org/1998/Math/MathML",
    "http://www.w3.org/1999/xhtml",
    "http://www.w3.org/2000/svg",
];
const NAMESPACE_PREFIXES: &[(&str, &str)] = &[
    ("http://creativecommons.org/ns#", "cc"),
    ("http://purl.org/dc/elements/1.1/", "dc"),
    (
        "http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd",
        "sodipodi",
    ),
    ("http://www.inkscape.org/namespaces/inkscape", "inkscape"),
    ("http://www.w3.org/1999/02/22-rdf-syntax-ns#", "rdf"),
    ("http://www.w3.org/1999/xlink", "xlink"),
    ("http://www.w3.org/XML/1998/namespace", "xml"),
];

/// Returns a canonical prefix of a namespace.
pub fn namespace_prefix(namespace: &str) -> Option<&'static str> {
    NAMESPACE_PREFIXES
        .iter()
        .find_map(|(other, prefix)| (*other == namespace).then_some(*prefix))
}

pub(crate) fn qualify_element_name(name: &QualName) -> String {
    qualify_name(name, DEFAULT_NAMESPACES)
}

// Prefixes bound to namespaces other than their canonical ones are replaced by
// the namespaces themselves so that such names never match names in the
// canonical namespaces.
//
// TODO Remove this hack.
pub(crate) fn qualify_attribute_name(name: &QualName) -> String {
    if let Some(prefix) = &name.prefix
        && !name.ns.is_empty()
        && namespace_prefix(&name.ns).is_none()
        && NAMESPACE_PREFIXES
            .iter()
            .any(|(_, other)| *other == &**prefix)
    {
        format!("{}:{}", name.ns, name.local)
    } else {
        qualify_name(name, &[""])
    }
}

fn qualify_name(name: &QualName, default_namespaces: &[&str]) -> String {
    if let Some(prefix) = namespace_prefix(&name.ns) {
        format!("{prefix}:{}", name.local)
    } else if default_namespaces.contains(&&*name.ns) {
        name.local.to_string()
    } else if let Some(prefix) = &name.prefix {
        format!("{prefix}:{}", name.local)
    } else {
        name.local.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn resolve_canonical_prefix() {
        assert_eq!(
            namespace_prefix("http://www.inkscape.org/namespaces/inkscape"),
            Some("inkscape")
        );
    }

    #[test]
    fn resolve_no_prefix_of_unknown_namespace() {
        assert_eq!(namespace_prefix("http://foo.example/"), None);
    }
}
