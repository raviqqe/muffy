use alloc::sync::Arc;
use core::ops::Deref;
use markup5ever_rcdom::NodeData;

pub enum Node {
    Element(Element),
    Text(String),
}

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

impl From<&markup5ever_rcdom::Node> for Node {
    fn from(node: Rc<markup5ever_rcdom::Node>) -> Self {
        match &node.data {
            NodeData::Element { name, attrs, .. } => Node::Element(Element::new(name, attrs)),
            NodeData::Text { contents } => Node::Text(contents.borrow().to_string()),
            _ => todo!(),
        }
    }
}
