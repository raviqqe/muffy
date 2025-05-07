use alloc::sync::Arc;
use core::ops::Deref;
use markup5ever_rcdom::NodeData;

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
}

#[derive(Debug, Eq, PartialEq)]
pub enum Node {
    Element(Element),
    Text(String),
}

impl Node {
    pub fn from_markup5ever(node: &markup5ever_rcdom::Node) -> Option<Self> {
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

#[derive(Debug, Eq, PartialEq)]
pub struct Element {
    name: String,
    attributes: Vec<(String, String)>,
    children: Vec<Arc<Node>>,
}

impl Element {
    pub const fn new(
        name: String,
        attributes: Vec<(String, String)>,
        children: Vec<Arc<Node>>,
    ) -> Self {
        Self {
            name,
            attributes,
            children,
        }
    }

    #[allow(clippy::missing_const_for_fn)]
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

impl From<Element> for Node {
    fn from(element: Element) -> Self {
        Self::Element(element)
    }
}
