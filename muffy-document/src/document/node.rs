use super::element::Element;
use alloc::sync::Arc;
use html5ever::{QualName, ns};
use markup5ever_rcdom::NodeData;

/// A node.
#[derive(Debug, Eq, PartialEq)]
pub enum Node {
    /// An element.
    Element(Element),
    /// A text.
    Text(String),
}

impl Node {
    pub(crate) fn from_markup5ever(node: &markup5ever_rcdom::Node) -> Option<Self> {
        match &node.data {
            NodeData::Element { name, attrs, .. } => Some(Self::Element(
                Element::new(
                    qualify_name(name),
                    attrs
                        .borrow()
                        .iter()
                        // Namespace declarations on foreign elements are not
                        // semantic attributes.
                        .filter(|attribute| attribute.name.ns != ns!(xmlns))
                        .map(|attribute| {
                            (qualify_name(&attribute.name), attribute.value.to_string())
                        })
                        .collect(),
                    node.children
                        .borrow()
                        .iter()
                        .flat_map(|node| Self::from_markup5ever(node))
                        .map(Arc::new)
                        .collect(),
                )
                .set_namespace((!name.ns.is_empty()).then(|| name.ns.to_string())),
            )),
            NodeData::Text { contents } => Some(Self::Text(contents.borrow().to_string())),
            NodeData::Comment { .. }
            | NodeData::Document
            | NodeData::Doctype { .. }
            | NodeData::ProcessingInstruction { .. } => None,
        }
    }
}

// Namespaces of languages validated without name prefixes.
const DEFAULT_NAMESPACES: &[&str] = &[
    "",
    "http://www.w3.org/1998/Math/MathML",
    "http://www.w3.org/1999/xhtml",
    "http://www.w3.org/2000/svg",
];
// Canonical prefixes of well-known namespaces.
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

fn qualify_name(name: &QualName) -> String {
    if let Some(prefix) = NAMESPACE_PREFIXES
        .iter()
        .find_map(|(namespace, prefix)| (*name.ns == **namespace).then_some(prefix))
    {
        format!("{prefix}:{}", name.local)
    } else if DEFAULT_NAMESPACES.contains(&&*name.ns) {
        name.local.to_string()
    } else if let Some(prefix) = &name.prefix {
        format!("{prefix}:{}", name.local)
    } else {
        name.local.to_string()
    }
}
