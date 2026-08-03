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

fn qualify_name(name: &QualName) -> String {
    if let Some(prefix) = &name.prefix {
        format!("{prefix}:{}", name.local)
    } else {
        name.local.to_string()
    }
}
