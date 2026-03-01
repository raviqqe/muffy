use super::node::Node;
use alloc::sync::Arc;
use core::ops::Deref;

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
