use super::document::Document;
use html5ever::{parse_document, tendril::TendrilSink};
use markup5ever_rcdom::RcDom;
use std::io;

/// Parses an HTML document.
pub fn parse(source: &str) -> Result<Document, io::Error> {
    parse_bytes(source.as_bytes())
}

/// Parses an HTML document from bytes.
pub fn parse_bytes(mut source: &[u8]) -> Result<Document, io::Error> {
    parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut source)
        .map(|dom| Document::from_markup5ever(&dom.document))
}

#[cfg(test)]
mod tests {
    use super::{
        super::{element::Element, node::Node},
        *,
    };
    use alloc::sync::Arc;
    use pretty_assertions::assert_eq;

    #[test]
    fn parse_empty_string() {
        assert_eq!(
            parse("").unwrap(),
            Document::new(vec![Arc::new(Node::Element(Element::new(
                "html".to_string(),
                vec![],
                vec![
                    Arc::new(Node::Element(Element::new(
                        "head".to_string(),
                        vec![],
                        vec![]
                    ))),
                    Arc::new(Node::Element(Element::new(
                        "body".to_string(),
                        vec![],
                        vec![]
                    ))),
                ],
            )))])
        );
    }

    #[test]
    fn parse_simple_html() {
        assert_eq!(
            parse("<html><body><p>Hello</p></body></html>").unwrap(),
            Document::new(vec![Arc::new(Node::Element(Element::new(
                "html".to_string(),
                vec![],
                vec![
                    Arc::new(Node::Element(Element::new(
                        "head".to_string(),
                        vec![],
                        vec![]
                    ))),
                    Arc::new(Node::Element(Element::new(
                        "body".to_string(),
                        vec![],
                        vec![Arc::new(Node::Element(Element::new(
                            "p".to_string(),
                            vec![],
                            vec![Arc::new(Node::Text("Hello".to_string()))],
                        )))],
                    ))),
                ],
            )))])
        );
    }

    #[test]
    fn parse_with_attributes() {
        assert_eq!(
            parse("<html><body><p class=\"foo\">Hello</p></body></html>").unwrap(),
            Document::new(vec![Arc::new(Node::Element(Element::new(
                "html".to_string(),
                vec![],
                vec![
                    Arc::new(Node::Element(Element::new(
                        "head".to_string(),
                        vec![],
                        vec![]
                    ))),
                    Arc::new(Node::Element(Element::new(
                        "body".to_string(),
                        vec![],
                        vec![Arc::new(Node::Element(Element::new(
                            "p".to_string(),
                            vec![("class".to_string(), "foo".to_string())],
                            vec![Arc::new(Node::Text("Hello".to_string()))],
                        )))],
                    ))),
                ],
            )))])
        );
    }

    #[test]
    fn ignore_comments() {
        assert_eq!(
            parse("<html><body><!-- comment --><p>Hello</p></body></html>").unwrap(),
            Document::new(vec![Arc::new(Node::Element(Element::new(
                "html".to_string(),
                vec![],
                vec![
                    Arc::new(Node::Element(Element::new(
                        "head".to_string(),
                        vec![],
                        vec![]
                    ))),
                    Arc::new(Node::Element(Element::new(
                        "body".to_string(),
                        vec![],
                        vec![Arc::new(Node::Element(Element::new(
                            "p".to_string(),
                            vec![],
                            vec![Arc::new(Node::Text("Hello".to_string()))],
                        )))],
                    ))),
                ],
            )))])
        );
    }
}
