use super::element::Element;
use alloc::sync::Arc;
use markup5ever_rcdom::NodeData;

/// A node.
#[derive(Debug, Eq, PartialEq)]
pub enum Node {
    Element(Element),
    Text(String),
}

impl Node {
    pub(crate) fn from_markup5ever(node: &markup5ever_rcdom::Node) -> Option<Self> {
        match &node.data {
            NodeData::Element { name, attrs, .. } => Some(Self::Element(Element::new(
                name.local.to_string(),
                attrs
                    .borrow()
                    .iter()
                    .map(|attribute| {
                        (
                            attribute.name.local.to_string(),
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
            ))),
            NodeData::Text { contents } => Some(Self::Text(contents.borrow().to_string())),
            NodeData::Comment { .. }
            | NodeData::Document
            | NodeData::Doctype { .. }
            | NodeData::ProcessingInstruction { .. } => None,
        }
    }
}
