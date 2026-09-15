use super::element::Element;

/// A node.
#[derive(Debug, Eq, PartialEq)]
pub enum Node {
    /// An element.
    Element(Element),
    /// A text.
    Text(String),
}
