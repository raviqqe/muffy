use alloc::sync::Arc;
use core::ops::Deref;
use markup5ever_rcdom::NodeData;

#[derive(Debug)]
pub struct Document {
    children: Vec<Arc<Node>>,
}

impl Document {
    pub fn from_markup5ever(node: &markup5ever_rcdom::Node) -> Self {
        if matches!(node.data, NodeData::Document) {
            Document {
                children: node
                    .children
                    .borrow()
                    .iter()
                    .flat_map(|node| Node::from_markup5ever(node))
                    .map(Arc::new)
                    .collect(),
            }
        } else {
            unreachable!()
        }
    }

    pub fn children(&self) -> impl Iterator<Item = &Node> {
        self.children.iter().map(Deref::deref)
    }
}

#[derive(Debug)]
pub enum Node {
    Element(Element),
    Text(String),
}

impl Node {
    pub fn from_markup5ever(node: &markup5ever_rcdom::Node) -> Option<Self> {
        match &node.data {
            NodeData::Element { name, attrs, .. } => Some(Node::Element(Element::new(
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
            NodeData::Text { contents } => Some(Node::Text(contents.borrow().to_string())),
            NodeData::Comment { .. }
            | NodeData::Document
            | NodeData::Doctype { .. }
            | NodeData::ProcessingInstruction { .. } => None,
        }
    }
}

#[derive(Debug)]
pub struct Element {
    name: String,
    attributes: Vec<(String, String)>,
    children: Vec<Arc<Node>>,
}

impl Element {
    pub fn new(name: String, attributes: Vec<(String, String)>, children: Vec<Arc<Node>>) -> Self {
        Self {
            name,
            attributes,
            children,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn attributes(&self) -> impl Iterator<Item = (&str, &str)> {
        self.attributes
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
    }

    pub fn children(&self) -> impl Iterator<Item = &Node> {
        self.children.iter().map(Deref::deref)
    }
}
