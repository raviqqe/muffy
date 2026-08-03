//! HTML documents.

use crate::document::Document;
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
    use super::*;
    use crate::document::{Element, Node};
    use alloc::sync::Arc;
    use pretty_assertions::assert_eq;

    const XHTML_NAMESPACE: &str = "http://www.w3.org/1999/xhtml";
    const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";

    fn element(
        namespace: &str,
        name: &str,
        attributes: Vec<(&str, &str)>,
        children: Vec<Arc<Node>>,
    ) -> Arc<Node> {
        Arc::new(Node::Element(
            Element::new(
                name.into(),
                attributes
                    .into_iter()
                    .map(|(name, value)| (name.into(), value.into()))
                    .collect(),
                children,
            )
            .set_namespace(Some(namespace.into())),
        ))
    }

    fn text(value: &str) -> Arc<Node> {
        Arc::new(Node::Text(value.into()))
    }

    #[test]
    fn parse_empty_string() {
        assert_eq!(
            parse("").unwrap(),
            Document::new(vec![element(
                XHTML_NAMESPACE,
                "html",
                vec![],
                vec![
                    element(XHTML_NAMESPACE, "head", vec![], vec![]),
                    element(XHTML_NAMESPACE, "body", vec![], vec![]),
                ],
            )])
        );
    }

    #[test]
    fn parse_simple_html() {
        assert_eq!(
            parse("<html><body><p>Hello</p></body></html>").unwrap(),
            Document::new(vec![element(
                XHTML_NAMESPACE,
                "html",
                vec![],
                vec![
                    element(XHTML_NAMESPACE, "head", vec![], vec![]),
                    element(
                        XHTML_NAMESPACE,
                        "body",
                        vec![],
                        vec![element(XHTML_NAMESPACE, "p", vec![], vec![text("Hello")])],
                    ),
                ],
            )])
        );
    }

    #[test]
    fn parse_with_attributes() {
        assert_eq!(
            parse("<html><body><p class=\"foo\">Hello</p></body></html>").unwrap(),
            Document::new(vec![element(
                XHTML_NAMESPACE,
                "html",
                vec![],
                vec![
                    element(XHTML_NAMESPACE, "head", vec![], vec![]),
                    element(
                        XHTML_NAMESPACE,
                        "body",
                        vec![],
                        vec![element(
                            XHTML_NAMESPACE,
                            "p",
                            vec![("class", "foo")],
                            vec![text("Hello")],
                        )],
                    ),
                ],
            )])
        );
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
            Document::new(vec![element(
                XHTML_NAMESPACE,
                "html",
                vec![],
                vec![
                    element(XHTML_NAMESPACE, "head", vec![], vec![]),
                    element(
                        XHTML_NAMESPACE,
                        "body",
                        vec![],
                        vec![element(
                            SVG_NAMESPACE,
                            "svg",
                            vec![],
                            vec![
                                element(
                                    SVG_NAMESPACE,
                                    "a",
                                    vec![("href", "/foo")],
                                    vec![element(SVG_NAMESPACE, "rect", vec![], vec![])],
                                ),
                                element(
                                    SVG_NAMESPACE,
                                    "image",
                                    vec![("xlink:href", "/bar.png")],
                                    vec![],
                                ),
                            ],
                        )],
                    ),
                ],
            )])
        );
    }

    #[test]
    fn ignore_comments() {
        assert_eq!(
            parse("<html><body><!-- comment --><p>Hello</p></body></html>").unwrap(),
            Document::new(vec![element(
                XHTML_NAMESPACE,
                "html",
                vec![],
                vec![
                    element(XHTML_NAMESPACE, "head", vec![], vec![]),
                    element(
                        XHTML_NAMESPACE,
                        "body",
                        vec![],
                        vec![element(XHTML_NAMESPACE, "p", vec![], vec![text("Hello")])],
                    ),
                ],
            )])
        );
    }
}
