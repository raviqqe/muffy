use super::{element::Element, node::Node};
use alloc::sync::Arc;
use core::ops::Deref;
use markup5ever_rcdom::NodeData;

/// A document.
#[derive(Debug, Eq, PartialEq)]
pub struct Document {
    children: Vec<Arc<Node>>,
}

impl Document {
    /// Creates a document.
    pub const fn new(children: Vec<Arc<Node>>) -> Self {
        Self { children }
    }

    /// Returns children.
    pub fn children(&self) -> impl Iterator<Item = &Node> {
        self.children.iter().map(Deref::deref)
    }

    /// Returns a base element.
    pub fn base(&self) -> Option<&str> {
        self.children()
            .find_map(|node| Self::find_base(node))
            .and_then(|element| {
                element
                    .attributes()
                    .find(|(key, _)| *key == "href")
                    .map(|(_, value)| value)
            })
    }

    fn find_base(node: &Node) -> Option<&Element> {
        match node {
            Node::Element(element) if element.name() == "base" => Some(element),
            Node::Element(element) => element.children().find_map(|node| Self::find_base(node)),
            _ => None,
        }
    }

    pub(crate) fn from_markup5ever(node: &markup5ever_rcdom::Node) -> Self {
        if matches!(node.data, NodeData::Document) {
            Self::new(
                node.children
                    .borrow()
                    .iter()
                    .flat_map(|node| Node::from_markup5ever(node))
                    .map(Arc::new)
                    .collect(),
            )
        } else {
            unreachable!()
        }
    }
}
