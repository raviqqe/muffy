//! XML documents.

use crate::document::Document;
use markup5ever_rcdom::RcDom;
use std::io;
use xml5ever::{driver::parse_document, tendril::TendrilSink};

/// Parses an XML document.
pub fn parse(source: &str) -> Result<Document, io::Error> {
    parse_bytes(source.as_bytes())
}

/// Parses an XML document from bytes.
pub fn parse_bytes(mut source: &[u8]) -> Result<Document, io::Error> {
    parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut source)
        .map(|dom| Document::from_markup5ever(&dom.document))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Element, Node};
    use alloc::sync::Arc;
    use pretty_assertions::assert_eq;

    #[test]
    fn parse_empty_string() {
        assert_eq!(parse("").unwrap(), Document::new(vec![]));
    }

    #[test]
    fn parse_svg_document() {
        assert_eq!(
            parse(concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">"#,
                r#"<a href="/foo"><rect/></a>"#,
                r#"<image xlink:href="/bar.png"/>"#,
                "</svg>"
            ))
            .unwrap(),
            Document::new(vec![Arc::new(Node::Element(Element::new(
                "svg".to_string(),
                // XML parsers consume namespace declarations.
                vec![],
                vec![
                    Arc::new(Node::Element(Element::new(
                        "a".to_string(),
                        vec![("href".to_string(), "/foo".to_string())],
                        vec![Arc::new(Node::Element(Element::new(
                            "rect".to_string(),
                            vec![],
                            vec![]
                        )))],
                    ))),
                    Arc::new(Node::Element(Element::new(
                        "image".to_string(),
                        vec![("href".to_string(), "/bar.png".to_string())],
                        vec![],
                    ))),
                ],
            )))])
        );
    }

    #[test]
    fn preserve_element_name_case() {
        assert_eq!(
            parse("<svg><foreignObject/></svg>").unwrap(),
            Document::new(vec![Arc::new(Node::Element(Element::new(
                "svg".to_string(),
                vec![],
                vec![Arc::new(Node::Element(Element::new(
                    "foreignObject".to_string(),
                    vec![],
                    vec![]
                )))],
            )))])
        );
    }

    #[test]
    fn keep_html_element_in_svg_element() {
        assert_eq!(
            parse("<svg><p>foo</p></svg>").unwrap(),
            Document::new(vec![Arc::new(Node::Element(Element::new(
                "svg".to_string(),
                vec![],
                vec![Arc::new(Node::Element(Element::new(
                    "p".to_string(),
                    vec![],
                    vec![Arc::new(Node::Text("foo".to_string()))],
                )))],
            )))])
        );
    }

    #[test]
    fn ignore_processing_instructions() {
        assert_eq!(
            parse(r#"<?xml version="1.0"?><svg/>"#).unwrap(),
            Document::new(vec![Arc::new(Node::Element(Element::new(
                "svg".to_string(),
                vec![],
                vec![],
            )))])
        );
    }
}
