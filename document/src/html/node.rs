use super::element::Element;
use alloc::sync::Arc;
use core::ops::Deref;
use markup5ever_rcdom::NodeData;

/// A document.
#[derive(Debug, Eq, PartialEq)]
pub struct Document {
    children: Vec<Arc<Node>>,
}

impl Document {
    pub const fn new(children: Vec<Arc<Node>>) -> Self {
        Self { children }
    }

    pub fn from_markup5ever(node: &markup5ever_rcdom::Node) -> Self {
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

    pub fn children(&self) -> impl Iterator<Item = &Node> {
        self.children.iter().map(Deref::deref)
    }

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
}

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
