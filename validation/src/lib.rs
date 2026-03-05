//! Document validation.

extern crate alloc;

use alloc::collections::BTreeMap;
use muffy_document::html::Element;
use muffy_validation_macro::html;

html! {}

/// A validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValidationError {
    /// An invalid element.
    InvalidElement(String),
    /// Invalid element details.
    InvalidElementDetails {
        /// Invalid attributes by name.
        attributes: BTreeMap<String, AttributeError>,
        /// Invalid children by name.
        children: BTreeMap<String, ChildError>,
    },
}

/// A validation attribute error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AttributeError {
    /// An invalid attribute.
    Invalid,
}

/// A validation child error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChildError {
    /// An invalid child.
    Invalid,
}

#[cfg(test)]
mod tests {
    use super::*;
    use muffy_document::html::Node;
    use std::sync::Arc;

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

    fn create_attributes(attribute_names: Vec<&str>) -> BTreeMap<String, AttributeError> {
        attribute_names
            .into_iter()
            .map(|attribute_name| {
                let attribute_name_string = attribute_name.to_string();
                (
                    attribute_name_string.clone(),
                    AttributeError::Invalid,
                )
            })
            .collect()
    }

    fn create_children(child_names: Vec<&str>) -> BTreeMap<String, ChildError> {
        child_names
            .into_iter()
            .map(|child_name| {
                let child_name_string = child_name.to_string();
                (
                    child_name_string.clone(),
                    ChildError::Invalid,
                )
            })
            .collect()
    }

    fn create_element_errors(
        attribute_names: Vec<&str>,
        child_names: Vec<&str>,
    ) -> ValidationError {
        ValidationError::InvalidElementDetails {
            attributes: create_attributes(attribute_names),
            children: create_children(child_names),
        }
    }

    #[test]
    fn validate_invalid_element_name() {
        let element = create_element("invalid", vec![], vec![]);

        assert_eq!(
            validate_element(&element),
            Err(ValidationError::InvalidElement("invalid".to_owned()))
        );
    }

    mod div {
        use super::*;

        #[test]
        fn validate_valid_element() {
            let element = create_element("div", vec![], vec![]);

            assert_eq!(validate_element(&element), Ok(()));
        }

        #[test]
        fn validate_valid_attributes() {
            let element = create_element("div", vec![("id", "foo"), ("class", "bar")], vec![]);

            assert_eq!(validate_element(&element), Ok(()));
        }

        #[test]
        fn validate_invalid_attribute() {
            let element = create_element("div", vec![("invalid", "foo")], vec![]);

            assert_eq!(
                validate_element(&element),
                Err(create_element_errors(vec!["invalid"], vec![]))
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
                validate_element(&element),
                Err(create_element_errors(
                    vec!["invalid-one", "invalid-two"],
                    vec![],
                ))
            );
        }

        #[test]
        fn validate_valid_child() {
            let element = create_element("div", vec![], vec![create_element("p", vec![], vec![])]);

            assert_eq!(validate_element(&element), Ok(()));
        }
    }

    mod p {
        use super::*;

        #[test]
        fn validate_valid_element() {
            let element = create_element("p", vec![], vec![]);

            assert_eq!(validate_element(&element), Ok(()));
        }

        #[test]
        fn validate_invalid_child() {
            let element = create_element("p", vec![], vec![create_element("div", vec![], vec![])]);

            assert_eq!(
                validate_element(&element),
                Err(create_element_errors(vec![], vec!["div"]))
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
                validate_element(&element),
                Err(create_element_errors(vec![], vec!["div", "table"]))
            );
        }
    }

    mod html {
        use super::*;

        #[test]
        fn validate_valid_element() {
            let element = create_element("html", vec![], vec![]);

            assert_eq!(validate_element(&element), Ok(()));
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

            assert_eq!(validate_element(&element), Ok(()));
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

            assert_eq!(validate_element(&element), Ok(()));
        }

        #[test]
        fn validate_invalid_child() {
            let element = create_element("head", vec![], vec![create_element("p", vec![], vec![])]);

            assert_eq!(
                validate_element(&element),
                Err(create_element_errors(vec![], vec!["p"]))
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
                validate_element(&element),
                Err(create_element_errors(vec![], vec!["div"]))
            );
        }
    }

    mod ul {
        use super::*;

        #[test]
        fn validate_valid_child() {
            let element = create_element("ul", vec![], vec![create_element("li", vec![], vec![])]);

            assert_eq!(validate_element(&element), Ok(()));
        }

        #[test]
        fn validate_invalid_child() {
            let element = create_element("ul", vec![], vec![create_element("p", vec![], vec![])]);

            assert_eq!(
                validate_element(&element),
                Err(create_element_errors(vec![], vec!["p"]))
            );
        }
    }

    mod table {
        use super::*;

        #[test]
        fn validate_valid_child() {
            let element =
                create_element("table", vec![], vec![create_element("tr", vec![], vec![])]);

            assert_eq!(validate_element(&element), Ok(()));
        }

        #[test]
        fn validate_invalid_child() {
            let element =
                create_element("table", vec![], vec![create_element("p", vec![], vec![])]);

            assert_eq!(
                validate_element(&element),
                Err(create_element_errors(vec![], vec!["p"]))
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

            assert_eq!(validate_element(&element), Ok(()));
        }
    }

    mod form {
        use super::*;

        #[test]
        fn validate_valid_attributes() {
            let element = create_element("form", vec![("action", "/"), ("method", "post")], vec![]);

            assert_eq!(validate_element(&element), Ok(()));
        }

        #[test]
        fn validate_valid_child() {
            let element = create_element(
                "form",
                vec![],
                vec![create_element("input", vec![], vec![])],
            );

            assert_eq!(validate_element(&element), Ok(()));
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

            assert_eq!(validate_element(&element), Ok(()));
        }
    }

    mod video {
        use super::*;

        #[test]
        fn validate_valid_attributes() {
            let element =
                create_element("video", vec![("src", "vid.mp4"), ("controls", "")], vec![]);

            assert_eq!(validate_element(&element), Ok(()));
        }

        #[test]
        fn validate_valid_child() {
            let element = create_element(
                "video",
                vec![],
                vec![create_element("track", vec![], vec![])],
            );

            assert_eq!(validate_element(&element), Ok(()));
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

            assert_eq!(validate_element(&element), Ok(()));
        }

        #[test]
        fn validate_valid_charset() {
            let element = create_element("meta", vec![("charset", "utf-8")], vec![]);

            assert_eq!(validate_element(&element), Ok(()));
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

            assert_eq!(validate_element(&element), Ok(()));
        }
    }
}
