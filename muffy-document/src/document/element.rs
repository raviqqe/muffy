use super::node::Node;
use alloc::sync::Arc;
use core::ops::Deref;

/// An element.
#[derive(Debug, Eq, PartialEq)]
pub struct Element {
    name: String,
    namespace: Option<String>,
    attributes: Vec<(String, String)>,
    children: Vec<Arc<Node>>,
}

impl Element {
    /// Creates an element.
    pub const fn new(
        name: String,
        attributes: Vec<(String, String)>,
        children: Vec<Arc<Node>>,
    ) -> Self {
        Self {
            name,
            namespace: None,
            attributes,
            children,
        }
    }

    /// Returns a name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns a namespace.
    pub fn namespace(&self) -> Option<&str> {
        self.namespace.as_deref()
    }

    /// Sets a namespace.
    pub fn set_namespace(mut self, namespace: Option<String>) -> Self {
        self.namespace = namespace;
        self
    }

    /// Returns attributes.
    pub fn attributes(&self) -> impl Iterator<Item = (&str, &str)> {
        self.attributes
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
    }

    /// Returns children.
    pub fn children(&self) -> impl Iterator<Item = &Node> {
        self.children.iter().map(Deref::deref)
    }
}

impl From<Element> for Node {
    fn from(element: Element) -> Self {
        Self::Element(element)
    }
}
