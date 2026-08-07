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
        .map(|dom| {
            Document::from_markup5ever(&dom.document).set_errors(
                dom.errors
                    .borrow()
                    .iter()
                    .map(ToString::to_string)
                    .collect(),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{Element, Node};
    use alloc::sync::Arc;
    use pretty_assertions::assert_eq;

    const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";

    fn element(
        namespace: Option<&str>,
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
            .set_namespace(namespace.map(Into::into)),
        ))
    }

    #[test]
    fn parse_empty_string() {
        assert_eq!(
            parse("").unwrap(),
            Document::new(vec![]).set_errors(vec!["Unexpected EOF in start phase".into()])
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
                Some(SVG_NAMESPACE),
                "svg",
                vec![],
                vec![
                    element(
                        Some(SVG_NAMESPACE),
                        "a",
                        vec![("href", "/foo")],
                        vec![element(Some(SVG_NAMESPACE), "rect", vec![], vec![])],
                    ),
                    element(
                        Some(SVG_NAMESPACE),
                        "image",
                        vec![("xlink:href", "/bar.png")],
                        vec![],
                    ),
                ],
            )])
        );
    }

    #[test]
    fn collect_error_on_multiple_root_elements() {
        let document = parse("<svg/><svg/>").unwrap();

        assert_eq!(document.children().count(), 1);
        assert_eq!(
            document.errors().collect::<Vec<_>>(),
            vec!["Unexpected element in end phase"]
        );
    }

    #[test]
    fn parse_element_without_namespace() {
        assert_eq!(
            parse("<svg><foreignObject/></svg>").unwrap(),
            Document::new(vec![element(
                None,
                "svg",
                vec![],
                vec![element(None, "foreignObject", vec![], vec![])],
            )])
        );
    }

    #[test]
    fn parse_prefixed_element() {
        assert_eq!(
            parse(concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg">"#,
                r#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"/>"#,
                "</svg>"
            ))
            .unwrap(),
            Document::new(vec![element(
                Some(SVG_NAMESPACE),
                "svg",
                vec![],
                vec![element(
                    Some("http://www.w3.org/1999/02/22-rdf-syntax-ns#"),
                    "rdf:RDF",
                    vec![],
                    vec![],
                )],
            )])
        );
    }

    #[test]
    fn canonicalize_element_prefix() {
        assert_eq!(
            parse(concat!(
                r#"<s:svg xmlns:s="http://www.w3.org/2000/svg">"#,
                "<s:circle/>",
                "</s:svg>"
            ))
            .unwrap(),
            Document::new(vec![element(
                Some(SVG_NAMESPACE),
                "svg",
                vec![],
                vec![element(Some(SVG_NAMESPACE), "circle", vec![], vec![])],
            )])
        );
    }

    #[test]
    fn canonicalize_attribute_prefix() {
        assert_eq!(
            parse(concat!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:x="http://www.w3.org/1999/xlink">"#,
                r#"<image x:href="/foo.png"/>"#,
                "</svg>"
            ))
            .unwrap(),
            Document::new(vec![element(
                Some(SVG_NAMESPACE),
                "svg",
                vec![],
                vec![element(
                    Some(SVG_NAMESPACE),
                    "image",
                    vec![("xlink:href", "/foo.png")],
                    vec![],
                )],
            )])
        );
    }

    #[test]
    fn keep_unknown_namespace_prefix() {
        assert_eq!(
            parse(r#"<f:thing xmlns:f="http://foo.example"/>"#).unwrap(),
            Document::new(vec![element(
                Some("http://foo.example"),
                "f:thing",
                vec![],
                vec![],
            )])
        );
    }

    #[test]
    fn keep_html_element_in_svg_element() {
        assert_eq!(
            parse(r#"<svg xmlns="http://www.w3.org/2000/svg"><p>foo</p></svg>"#).unwrap(),
            Document::new(vec![element(
                Some(SVG_NAMESPACE),
                "svg",
                vec![],
                vec![element(
                    Some(SVG_NAMESPACE),
                    "p",
                    vec![],
                    vec![Arc::new(Node::Text("foo".into()))],
                )],
            )])
        );
    }

    #[test]
    fn ignore_processing_instructions() {
        assert_eq!(
            parse(r#"<?xml version="1.0"?><svg/>"#).unwrap(),
            Document::new(vec![element(None, "svg", vec![], vec![])])
        );
    }
}
