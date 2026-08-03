//! Document validation.

extern crate alloc;

mod attribute_set;
mod content;
mod error;
mod rule;
mod validation;
mod variant;

pub use self::error::*;
use self::{
    attribute_set::AttributeSet, content::Content, rule::Rule, validation::validate_rule,
    variant::Variant,
};
use muffy_document::document::Element;
use muffy_validation_macro::html;
use regex::Regex;

html! {}

/// Validates an SVG element.
///
/// The HTML and SVG schemas are composed into one document schema.
pub fn validate_svg_element(
    element: &Element,
    ignored_attributes: &[Regex],
    ignored_elements: &[Regex],
) -> Result<(), MarkupError> {
    validate_html_element(element, ignored_attributes, ignored_elements)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::sync::Arc;
    use muffy_document::document::Node;
    use regex::Regex;

    fn create_element(
        name: &str,
        attributes: Vec<(&str, &str)>,
        children: Vec<Element>,
    ) -> Element {
        Element::new(
            name.to_owned(),
            attributes
                .into_iter()
                .map(|(k, v)| (k.to_owned(), v.to_owned()))
                .collect(),
            children
                .into_iter()
                .map(|e| Arc::new(Node::Element(e)))
                .collect(),
        )
    }

    #[test]
    fn validate_invalid_element_name() {
        let element = create_element("invalid", vec![], vec![]);

        assert_eq!(
            validate_html_element(&element, &[], &[]),
            Err(MarkupError::UnknownTag("invalid".to_owned()))
        );
    }

    mod div {
        use super::*;

        #[test]
        fn validate_valid_attribute_name_prefix() {
            let element = create_element("div", vec![("lang", "en"), ("xml:lang", "en")], vec![]);

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_valid_element() {
            let element = create_element("div", vec![], vec![]);

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_valid_attributes() {
            let element = create_element("div", vec![("id", "foo"), ("class", "bar")], vec![]);

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_invalid_attribute() {
            let element = create_element("div", vec![("invalid", "foo")], vec![]);

            assert_eq!(
                validate_html_element(&element, &[], &[]),
                Err(MarkupError::InvalidElement {
                    invalid_attributes: [("invalid".into(), [AttributeError::NotAllowed].into())]
                        .into(),
                    invalid_children: Default::default(),
                    missing_attributes: Default::default(),
                    missing_children: Default::default(),
                })
            );
        }

        #[test]
        fn validate_multiple_invalid_attributes() {
            let element = create_element(
                "div",
                vec![("invalid-one", "foo"), ("invalid-two", "bar")],
                vec![],
            );

            assert_eq!(
                validate_html_element(&element, &[], &[]),
                Err(MarkupError::InvalidElement {
                    invalid_attributes: [
                        ("invalid-one".into(), [AttributeError::NotAllowed].into()),
                        ("invalid-two".into(), [AttributeError::NotAllowed].into()),
                    ]
                    .into(),
                    invalid_children: Default::default(),
                    missing_attributes: Default::default(),
                    missing_children: Default::default(),
                })
            );
        }

        #[test]
        fn validate_ignored_attribute_regex() {
            let element = create_element("div", vec![("data-foo", "bar")], vec![]);

            assert_eq!(
                validate_html_element(&element, &[Regex::new("^data-.*$").unwrap()], &[]),
                Ok(())
            );
        }

        #[test]
        fn validate_ignored_element_regex() {
            let element = create_element(
                "div",
                vec![],
                vec![create_element("custom-element-123", vec![], vec![])],
            );

            assert_eq!(
                validate_html_element(&element, &[], &[Regex::new("^custom-element-.*$").unwrap()]),
                Ok(())
            );
        }

        #[test]
        fn validate_ignored_unknown_tag_regex() {
            let element = create_element("custom-element-456", vec![], vec![]);

            assert_eq!(
                validate_html_element(&element, &[], &[Regex::new("^custom-element-.*$").unwrap()]),
                Ok(())
            );
        }

        #[test]
        fn validate_ignored_known_tag_regex() {
            let element = create_element("div", vec![("invalid", "foo")], vec![]);

            assert_eq!(
                validate_html_element(&element, &[], &[Regex::new("^div$").unwrap()]),
                Ok(())
            );
        }

        #[test]
        fn validate_non_ignored_known_tag_regex() {
            let element = create_element("div", vec![("invalid", "foo")], vec![]);

            assert_eq!(
                validate_html_element(&element, &[], &[Regex::new("^span$").unwrap()]),
                Err(MarkupError::InvalidElement {
                    invalid_attributes: [("invalid".into(), [AttributeError::NotAllowed].into())]
                        .into(),
                    invalid_children: Default::default(),
                    missing_attributes: Default::default(),
                    missing_children: Default::default(),
                })
            );
        }

        #[test]
        fn validate_valid_child() {
            let element = create_element("div", vec![], vec![create_element("p", vec![], vec![])]);

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_valid_svg_child() {
            let element =
                create_element("div", vec![], vec![create_element("svg", vec![], vec![])]);

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }
    }

    mod p {
        use super::*;

        #[test]
        fn validate_valid_element() {
            let element = create_element("p", vec![], vec![]);

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_valid_text() {
            let element = Element::new(
                "p".into(),
                vec![],
                vec![Arc::new(Node::Text("hello".into()))],
            );

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_invalid_child() {
            let element = create_element("p", vec![], vec![create_element("div", vec![], vec![])]);

            assert_eq!(
                validate_html_element(&element, &[], &[]),
                Err(MarkupError::InvalidElement {
                    invalid_attributes: Default::default(),
                    invalid_children: [("div".into(), [ChildError::NotAllowed].into())].into(),
                    missing_attributes: Default::default(),
                    missing_children: Default::default(),
                })
            );
        }

        #[test]
        fn validate_multiple_invalid_children() {
            let element = create_element(
                "p",
                vec![],
                vec![
                    create_element("div", vec![], vec![]),
                    create_element("table", vec![], vec![]),
                ],
            );

            assert_eq!(
                validate_html_element(&element, &[], &[]),
                Err(MarkupError::InvalidElement {
                    invalid_attributes: Default::default(),
                    invalid_children: [
                        ("div".into(), [ChildError::NotAllowed].into()),
                        ("table".into(), [ChildError::NotAllowed].into()),
                    ]
                    .into(),
                    missing_attributes: Default::default(),
                    missing_children: Default::default(),
                })
            );
        }

        #[test]
        fn validate_ignored_known_tag_regex() {
            let element = create_element("p", vec![], vec![create_element("div", vec![], vec![])]);

            assert_eq!(
                validate_html_element(&element, &[], &[Regex::new("^p$").unwrap()]),
                Ok(())
            );
        }
    }

    mod a {
        use super::*;

        #[test]
        fn validate_valid_link() {
            let element = create_element("a", vec![("href", "/")], vec![]);

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_valid_prefixed_link() {
            let element = create_element("a", vec![("xlink:href", "/")], vec![]);

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_valid_placeholder_link() {
            let element = create_element("a", vec![], vec![]);

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_missing_href() {
            let element = create_element("a", vec![("download", "")], vec![]);

            assert_eq!(
                validate_html_element(&element, &[], &[]),
                Err(MarkupError::InvalidElement {
                    invalid_attributes: Default::default(),
                    invalid_children: Default::default(),
                    missing_attributes: ["href".into()].into(),
                    missing_children: Default::default(),
                })
            );
        }
    }

    mod mark {
        use super::*;

        #[test]
        fn validate_valid_element() {
            let element = create_element("mark", vec![], vec![]);

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_valid_child() {
            let element =
                create_element("mark", vec![], vec![create_element("span", vec![], vec![])]);

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_invalid_child() {
            let element =
                create_element("mark", vec![], vec![create_element("div", vec![], vec![])]);

            assert_eq!(
                validate_html_element(&element, &[], &[]),
                Err(MarkupError::InvalidElement {
                    invalid_attributes: Default::default(),
                    invalid_children: [("div".into(), [ChildError::NotAllowed].into())].into(),
                    missing_attributes: Default::default(),
                    missing_children: Default::default(),
                })
            );
        }
    }

    mod html {
        use super::*;

        #[test]
        fn validate_missing_children() {
            let element = create_element("html", vec![], vec![]);

            assert_eq!(
                validate_html_element(&element, &[], &[]),
                Err(MarkupError::InvalidElement {
                    invalid_attributes: Default::default(),
                    invalid_children: Default::default(),
                    missing_attributes: Default::default(),
                    missing_children: ["head".into()].into(),
                })
            );
        }

        #[test]
        fn validate_valid_children() {
            let element = create_element(
                "html",
                vec![],
                vec![
                    create_element("head", vec![], vec![]),
                    create_element("body", vec![], vec![]),
                ],
            );

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_misplaced_children() {
            let element = create_element(
                "html",
                vec![],
                vec![
                    create_element("body", vec![], vec![]),
                    create_element("head", vec![], vec![]),
                ],
            );

            assert_eq!(
                validate_html_element(&element, &[], &[]),
                Err(MarkupError::InvalidElement {
                    invalid_attributes: Default::default(),
                    invalid_children: [("body".into(), [ChildError::Misplaced].into())].into(),
                    missing_attributes: Default::default(),
                    missing_children: Default::default(),
                })
            );
        }
    }

    mod head {
        use super::*;

        #[test]
        fn validate_valid_child() {
            let element = create_element(
                "head",
                vec![],
                vec![create_element("title", vec![], vec![])],
            );

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_missing_title() {
            let element = create_element("head", vec![], vec![]);

            assert_eq!(
                validate_html_element(&element, &[], &[]),
                Err(MarkupError::InvalidElement {
                    invalid_attributes: Default::default(),
                    invalid_children: Default::default(),
                    missing_attributes: Default::default(),
                    missing_children: ["title".into()].into(),
                })
            );
        }

        #[test]
        fn validate_invalid_child() {
            let element = create_element("head", vec![], vec![create_element("p", vec![], vec![])]);

            assert_eq!(
                validate_html_element(&element, &[], &[]),
                Err(MarkupError::InvalidElement {
                    invalid_attributes: Default::default(),
                    invalid_children: [("p".into(), [ChildError::NotAllowed].into())].into(),
                    missing_attributes: Default::default(),
                    missing_children: ["title".into()].into(),
                })
            );
        }
    }

    mod title {
        use super::*;

        #[test]
        fn validate_invalid_child() {
            let element =
                create_element("title", vec![], vec![create_element("div", vec![], vec![])]);

            assert_eq!(
                validate_html_element(&element, &[], &[]),
                Err(MarkupError::InvalidElement {
                    invalid_attributes: Default::default(),
                    invalid_children: [("div".into(), [ChildError::NotAllowed].into())].into(),
                    missing_attributes: Default::default(),
                    missing_children: Default::default(),
                })
            );
        }
    }

    mod ul {
        use super::*;

        #[test]
        fn validate_valid_child() {
            let element = create_element("ul", vec![], vec![create_element("li", vec![], vec![])]);

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_whitespace_text() {
            let element = Element::new(
                "ul".into(),
                vec![],
                vec![
                    Arc::new(Node::Text("\n    ".into())),
                    Arc::new(Node::Element(create_element("li", vec![], vec![]))),
                ],
            );

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_invalid_text() {
            let element = Element::new(
                "ul".into(),
                vec![],
                vec![
                    Arc::new(Node::Text("orphan".into())),
                    Arc::new(Node::Element(create_element("li", vec![], vec![]))),
                ],
            );

            assert_eq!(
                validate_html_element(&element, &[], &[]),
                Err(MarkupError::InvalidElement {
                    invalid_attributes: Default::default(),
                    invalid_children: [("#text".into(), [ChildError::NotAllowed].into())].into(),
                    missing_attributes: Default::default(),
                    missing_children: Default::default(),
                })
            );
        }

        #[test]
        fn validate_invalid_child() {
            let element = create_element("ul", vec![], vec![create_element("p", vec![], vec![])]);

            assert_eq!(
                validate_html_element(&element, &[], &[]),
                Err(MarkupError::InvalidElement {
                    invalid_attributes: Default::default(),
                    invalid_children: [("p".into(), [ChildError::NotAllowed].into())].into(),
                    missing_attributes: Default::default(),
                    missing_children: Default::default(),
                })
            );
        }
    }

    mod table {
        use super::*;

        #[test]
        fn validate_valid_child() {
            let element =
                create_element("table", vec![], vec![create_element("tr", vec![], vec![])]);

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_valid_ordered_children() {
            let element = create_element(
                "table",
                vec![],
                vec![
                    create_element("caption", vec![], vec![]),
                    create_element("thead", vec![], vec![]),
                    create_element("tbody", vec![], vec![]),
                    create_element("tfoot", vec![], vec![]),
                ],
            );

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_misplaced_child() {
            let element = create_element(
                "table",
                vec![],
                vec![
                    create_element("tfoot", vec![], vec![]),
                    create_element("thead", vec![], vec![]),
                ],
            );

            assert_eq!(
                validate_html_element(&element, &[], &[]),
                Err(MarkupError::InvalidElement {
                    invalid_attributes: Default::default(),
                    invalid_children: [("thead".into(), [ChildError::Misplaced].into())].into(),
                    missing_attributes: Default::default(),
                    missing_children: Default::default(),
                })
            );
        }

        #[test]
        fn validate_invalid_child() {
            let element =
                create_element("table", vec![], vec![create_element("p", vec![], vec![])]);

            assert_eq!(
                validate_html_element(&element, &[], &[]),
                Err(MarkupError::InvalidElement {
                    invalid_attributes: Default::default(),
                    invalid_children: [("p".into(), [ChildError::NotAllowed].into())].into(),
                    missing_attributes: Default::default(),
                    missing_children: Default::default(),
                })
            );
        }
    }

    mod tr {
        use super::*;

        #[test]
        fn validate_valid_children() {
            let element = create_element(
                "tr",
                vec![],
                vec![
                    create_element("th", vec![], vec![]),
                    create_element("td", vec![], vec![]),
                ],
            );

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }
    }

    mod form {
        use super::*;

        #[test]
        fn validate_valid_attributes() {
            let element = create_element("form", vec![("action", "/"), ("method", "post")], vec![]);

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_valid_child() {
            let element = create_element(
                "form",
                vec![],
                vec![create_element("input", vec![], vec![])],
            );

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }
    }

    mod img {
        use super::*;

        #[test]
        fn validate_valid_attributes() {
            let element = create_element(
                "img",
                vec![("src", "img.png"), ("alt", "description")],
                vec![],
            );

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }
    }

    mod picture {
        use super::*;

        #[test]
        fn validate_missing_child() {
            let element = create_element("picture", vec![], vec![]);

            assert_eq!(
                validate_html_element(&element, &[], &[]),
                Err(MarkupError::InvalidElement {
                    invalid_attributes: Default::default(),
                    invalid_children: Default::default(),
                    missing_attributes: Default::default(),
                    missing_children: ["img".into()].into(),
                })
            );
        }

        #[test]
        fn validate_valid_children() {
            let element = create_element(
                "picture",
                vec![],
                vec![
                    create_element("source", vec![], vec![]),
                    create_element("img", vec![], vec![]),
                ],
            );

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_misplaced_child() {
            let element = create_element(
                "picture",
                vec![],
                vec![
                    create_element("img", vec![], vec![]),
                    create_element("source", vec![], vec![]),
                ],
            );

            assert_eq!(
                validate_html_element(&element, &[], &[]),
                Err(MarkupError::InvalidElement {
                    invalid_attributes: Default::default(),
                    invalid_children: [("source".into(), [ChildError::Misplaced].into())].into(),
                    missing_attributes: Default::default(),
                    missing_children: Default::default(),
                })
            );
        }

        #[test]
        fn validate_invalid_child() {
            let element =
                create_element("picture", vec![], vec![create_element("p", vec![], vec![])]);

            assert_eq!(
                validate_html_element(&element, &[], &[]),
                Err(MarkupError::InvalidElement {
                    invalid_attributes: Default::default(),
                    invalid_children: [("p".into(), [ChildError::NotAllowed].into())].into(),
                    missing_attributes: Default::default(),
                    missing_children: ["img".into()].into(),
                })
            );
        }
    }

    mod video {
        use super::*;

        #[test]
        fn validate_valid_attributes() {
            let element =
                create_element("video", vec![("src", "vid.mp4"), ("controls", "")], vec![]);

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_valid_child() {
            let element = create_element(
                "video",
                vec![],
                vec![create_element("track", vec![], vec![])],
            );

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }
    }

    mod meta {
        use super::*;

        #[test]
        fn validate_valid_name_content() {
            let element = create_element(
                "meta",
                vec![("name", "description"), ("content", "stuff")],
                vec![],
            );

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_valid_charset() {
            let element = create_element("meta", vec![("charset", "utf-8")], vec![]);

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_valid_property() {
            let element = create_element(
                "meta",
                vec![("property", "og:image"), ("content", "image.png")],
                vec![],
            );

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_missing_content() {
            let element = create_element("meta", vec![("name", "description")], vec![]);

            assert_eq!(
                validate_html_element(&element, &[], &[]),
                Err(MarkupError::InvalidElement {
                    invalid_attributes: Default::default(),
                    invalid_children: Default::default(),
                    missing_attributes: ["content".into()].into(),
                    missing_children: Default::default(),
                })
            );
        }

        #[test]
        fn validate_conflicting_charset() {
            let element = create_element(
                "meta",
                vec![
                    ("charset", "utf-8"),
                    ("name", "description"),
                    ("content", "stuff"),
                ],
                vec![],
            );

            assert_eq!(
                validate_html_element(&element, &[], &[]),
                Err(MarkupError::InvalidElement {
                    invalid_attributes: [("charset".into(), [AttributeError::Conflict].into())]
                        .into(),
                    invalid_children: Default::default(),
                    missing_attributes: Default::default(),
                    missing_children: Default::default(),
                })
            );
        }
    }

    mod link {
        use super::*;

        #[test]
        fn validate_valid_attributes() {
            let element = create_element(
                "link",
                vec![("rel", "stylesheet"), ("href", "style.css")],
                vec![],
            );

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_valid_rel_without_href() {
            let element = create_element("link", vec![("rel", "preload")], vec![]);

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_missing_rel() {
            let element = create_element("link", vec![("href", "style.css")], vec![]);

            // The schema alternatively requires either the `rel` attribute or
            // the `itemprop` attribute, and one minimal diagnosis is reported.
            assert_eq!(
                validate_html_element(&element, &[], &[]),
                Err(MarkupError::InvalidElement {
                    invalid_attributes: Default::default(),
                    invalid_children: Default::default(),
                    missing_attributes: ["itemprop".into()].into(),
                    missing_children: Default::default(),
                })
            );
        }
    }

    mod svg {
        use super::*;

        #[test]
        fn validate_valid_element() {
            let element = create_element("svg", vec![], vec![]);

            assert_eq!(validate_svg_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_valid_attributes() {
            let element = create_element(
                "svg",
                vec![
                    ("version", "1.1"),
                    ("viewBox", "0 0 10 10"),
                    ("width", "10"),
                    ("height", "10"),
                ],
                vec![],
            );

            assert_eq!(validate_svg_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_valid_aria_attributes() {
            let element = create_element(
                "svg",
                vec![("role", "img"), ("aria-label", "description")],
                vec![],
            );

            assert_eq!(validate_svg_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_valid_child() {
            let element = create_element(
                "svg",
                vec![],
                vec![create_element("circle", vec![("r", "1")], vec![])],
            );

            assert_eq!(validate_svg_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_valid_text_child() {
            let element = Element::new(
                "text".into(),
                vec![],
                vec![Arc::new(Node::Text("hello".into()))],
            );

            assert_eq!(validate_svg_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_valid_camel_case_element() {
            let element = create_element(
                "linearGradient",
                vec![("id", "gradient")],
                vec![create_element("stop", vec![("offset", "0")], vec![])],
            );

            assert_eq!(validate_svg_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_valid_link() {
            let element = create_element(
                "a",
                vec![("href", "/")],
                vec![create_element("rect", vec![], vec![])],
            );

            assert_eq!(validate_svg_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_valid_image_link() {
            let element = create_element("image", vec![("href", "/foo.png")], vec![]);

            assert_eq!(validate_svg_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_valid_prefixed_image_link() {
            let element = create_element("image", vec![("xlink:href", "/foo.png")], vec![]);

            assert_eq!(validate_svg_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_valid_foreign_object() {
            let element = create_element(
                "foreignObject",
                vec![],
                vec![create_element("div", vec![], vec![])],
            );

            assert_eq!(validate_svg_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_invalid_foreign_object_child() {
            let element = create_element(
                "foreignObject",
                vec![],
                vec![create_element("title", vec![], vec![])],
            );

            assert_eq!(
                validate_svg_element(&element, &[], &[]),
                Err(MarkupError::InvalidElement {
                    invalid_attributes: Default::default(),
                    invalid_children: [("title".into(), [ChildError::NotAllowed].into())].into(),
                    missing_attributes: Default::default(),
                    missing_children: Default::default(),
                })
            );
        }

        #[test]
        fn validate_invalid_element_name() {
            let element = create_element("invalid", vec![], vec![]);

            assert_eq!(
                validate_svg_element(&element, &[], &[]),
                Err(MarkupError::UnknownTag("invalid".to_owned()))
            );
        }

        #[test]
        fn validate_invalid_attribute() {
            let element = create_element("circle", vec![("invalid", "foo")], vec![]);

            assert_eq!(
                validate_svg_element(&element, &[], &[]),
                Err(MarkupError::InvalidElement {
                    invalid_attributes: [("invalid".into(), [AttributeError::NotAllowed].into())]
                        .into(),
                    invalid_children: Default::default(),
                    missing_attributes: Default::default(),
                    missing_children: Default::default(),
                })
            );
        }

        #[test]
        fn validate_invalid_child() {
            let element =
                create_element("svg", vec![], vec![create_element("html", vec![], vec![])]);

            assert_eq!(
                validate_svg_element(&element, &[], &[]),
                Err(MarkupError::InvalidElement {
                    invalid_attributes: Default::default(),
                    invalid_children: [("html".into(), [ChildError::NotAllowed].into())].into(),
                    missing_attributes: Default::default(),
                    missing_children: Default::default(),
                })
            );
        }

        #[test]
        fn validate_missing_attribute() {
            let element = create_element("animate", vec![], vec![]);

            assert_eq!(
                validate_svg_element(&element, &[], &[]),
                Err(MarkupError::InvalidElement {
                    invalid_attributes: Default::default(),
                    invalid_children: Default::default(),
                    missing_attributes: ["attributeName".into()].into(),
                    missing_children: Default::default(),
                })
            );
        }

        #[test]
        fn validate_ignored_attribute_regex() {
            let element = create_element("circle", vec![("data-foo", "bar")], vec![]);

            assert_eq!(
                validate_svg_element(&element, &[Regex::new("^data-.*$").unwrap()], &[]),
                Ok(())
            );
        }

        #[test]
        fn validate_ignored_element_regex() {
            let element = create_element("foreignObject", vec![], vec![]);

            assert_eq!(
                validate_svg_element(&element, &[], &[Regex::new("^foreignObject$").unwrap()]),
                Ok(())
            );
        }
    }

    mod noscript {
        use super::*;

        #[test]
        fn validate_valid_element() {
            let element = create_element("noscript", vec![], vec![]);

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_valid_flow_child() {
            let element = create_element(
                "noscript",
                vec![],
                vec![create_element("div", vec![], vec![])],
            );

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_valid_head_child() {
            let element = create_element(
                "noscript",
                vec![],
                vec![create_element("link", vec![], vec![])],
            );

            assert_eq!(validate_html_element(&element, &[], &[]), Ok(()));
        }

        #[test]
        fn validate_invalid_child() {
            let element = create_element(
                "noscript",
                vec![],
                vec![create_element("title", vec![], vec![])],
            );

            assert_eq!(
                validate_html_element(&element, &[], &[]),
                Err(MarkupError::InvalidElement {
                    invalid_attributes: Default::default(),
                    invalid_children: [("title".into(), [ChildError::NotAllowed].into())].into(),
                    missing_attributes: Default::default(),
                    missing_children: Default::default(),
                })
            );
        }
    }
}
