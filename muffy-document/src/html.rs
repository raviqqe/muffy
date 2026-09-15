//! HTML documents.

use crate::document::{Document, DocumentSink};
use html5ever::{parse_document, tendril::TendrilSink};
use std::io;

/// Parses an HTML document.
pub fn parse(source: &str) -> Result<Document, io::Error> {
    parse_bytes(source.as_bytes())
}

/// Parses an HTML document from bytes.
pub fn parse_bytes(mut source: &[u8]) -> Result<Document, io::Error> {
    parse_document(DocumentSink::default(), Default::default())
        .from_utf8()
        .read_from(&mut source)
        .map(|(document, _)| document)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Element, Node};
    use alloc::sync::Arc;
    use pretty_assertions::assert_eq;

    const MATHML_NAMESPACE: &str = "http://www.w3.org/1998/Math/MathML";
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

    #[test]
    fn keep_texts_separated_by_comment() {
        assert_eq!(
            parse("<p>foo<!-- comment -->bar</p>").unwrap(),
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
                            vec![],
                            vec![text("foo"), text("bar")],
                        )],
                    ),
                ],
            )])
        );
    }

    #[test]
    fn merge_adjacent_texts() {
        assert_eq!(
            parse("<p>foo</b>bar</p>").unwrap(),
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
                        vec![element(XHTML_NAMESPACE, "p", vec![], vec![text("foobar")])],
                    ),
                ],
            )])
        );
    }

    #[test]
    fn foster_parent_text() {
        assert_eq!(
            parse("foo<table>bar</table>").unwrap(),
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
                        vec![
                            text("foobar"),
                            element(XHTML_NAMESPACE, "table", vec![], vec![]),
                        ],
                    ),
                ],
            )])
        );
    }

    #[test]
    fn foster_parent_element() {
        assert_eq!(
            parse("<table><p>foo</p></table>").unwrap(),
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
                        vec![
                            element(XHTML_NAMESPACE, "p", vec![], vec![text("foo")]),
                            element(XHTML_NAMESPACE, "table", vec![], vec![]),
                        ],
                    ),
                ],
            )])
        );
    }

    #[test]
    fn split_formatting_element_across_paragraph() {
        assert_eq!(
            parse("<b>foo<p>bar</b>baz</p>").unwrap(),
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
                        vec![
                            element(XHTML_NAMESPACE, "b", vec![], vec![text("foo")]),
                            element(
                                XHTML_NAMESPACE,
                                "p",
                                vec![],
                                vec![
                                    element(XHTML_NAMESPACE, "b", vec![], vec![text("bar")]),
                                    text("baz"),
                                ],
                            ),
                        ],
                    ),
                ],
            )])
        );
    }

    #[test]
    fn merge_attributes_of_duplicate_html_element() {
        assert_eq!(
            parse(r#"<html lang="en"><body><html lang="ja" class="foo">"#).unwrap(),
            Document::new(vec![element(
                XHTML_NAMESPACE,
                "html",
                vec![("lang", "en"), ("class", "foo")],
                vec![
                    element(XHTML_NAMESPACE, "head", vec![], vec![]),
                    element(XHTML_NAMESPACE, "body", vec![], vec![]),
                ],
            )])
        );
    }

    #[test]
    fn ignore_template_contents() {
        assert_eq!(
            parse("<template><p>foo</p></template>").unwrap(),
            Document::new(vec![element(
                XHTML_NAMESPACE,
                "html",
                vec![],
                vec![
                    element(
                        XHTML_NAMESPACE,
                        "head",
                        vec![],
                        vec![element(XHTML_NAMESPACE, "template", vec![], vec![])],
                    ),
                    element(XHTML_NAMESPACE, "body", vec![], vec![]),
                ],
            )])
        );
    }

    #[test]
    fn parse_html_in_mathml_annotation() {
        assert_eq!(
            parse(concat!(
                "<math>",
                r#"<annotation-xml encoding="text/html"><div>foo</div></annotation-xml>"#,
                "</math>"
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
                            MATHML_NAMESPACE,
                            "math",
                            vec![],
                            vec![element(
                                MATHML_NAMESPACE,
                                "annotation-xml",
                                vec![("encoding", "text/html")],
                                vec![element(XHTML_NAMESPACE, "div", vec![], vec![text("foo")])],
                            )],
                        )],
                    ),
                ],
            )])
        );
    }
}
