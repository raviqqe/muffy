use super::{
    element::Element,
    namespace::{qualify_attribute_name, qualify_element_name},
};
use alloc::sync::Arc;
use html5ever::ns;
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
                    qualify_element_name(name),
                    attrs
                        .borrow()
                        .iter()
                        // Namespace declarations on foreign elements are not
                        // semantic attributes.
                        .filter(|attribute| attribute.name.ns != ns!(xmlns))
                        .map(|attribute| {
                            (
                                qualify_attribute_name(&attribute.name),
                                attribute.value.to_string(),
                            )
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
