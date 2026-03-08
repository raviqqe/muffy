//! Document validation.

extern crate alloc;

use alloc::collections::{BTreeMap, BTreeSet};
use core::fmt::{self, Display, Formatter};
use muffy_document::html::Element;
use muffy_validation_macro::html;

html! {}

/// A validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValidationError {
    /// An invalid tag.
    InvalidTag(String),
    /// Invalid element.
    InvalidElement {
        /// Invalid attributes by name.
        attributes: BTreeMap<String, BTreeSet<AttributeError>>,
        /// Invalid children by name.
        children: BTreeMap<String, BTreeSet<ChildError>>,
    },
}

impl Display for ValidationError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTag(tag) => write!(formatter, "invalid tag \"{tag}\""),
            Self::InvalidElement {
                attributes,
                children,
            } => {
                if !attributes.is_empty() {
                    write!(formatter, "invalid attributes: ")?;

                    for (index, (name, errors)) in attributes.iter().enumerate() {
                        if index > 0 {
                            write!(formatter, ", ")?;
                        }

                        write!(formatter, "{name} (")?;

                        for (index, error) in errors.iter().enumerate() {
                            if index > 0 {
                                write!(formatter, ", ")?;
                            }

                            write!(formatter, "{error}")?;
                        }

                        write!(formatter, ")")?;
                    }
                }

                if !children.is_empty() {
                    if !attributes.is_empty() {
                        write!(formatter, ", ")?;
                    }

                    write!(formatter, "invalid children: ")?;

                    for (index, (name, errors)) in children.iter().enumerate() {
                        if index > 0 {
                            write!(formatter, ", ")?;
                        }

                        write!(formatter, "{name} (")?;

                        for (index, error) in errors.iter().enumerate() {
                            if index > 0 {
                                write!(formatter, ", ")?;
                            }

                            write!(formatter, "{error}")?;
                        }

                        write!(formatter, ")")?;
                    }
                }

                Ok(())
            }
        }
    }
}

/// A validation attribute error.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AttributeError {
    /// An invalid attribute.
    Invalid,
}

impl Display for AttributeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid => write!(formatter, "invalid"),
        }
    }
}

/// A validation child error.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ChildError {
    /// An invalid child.
    Invalid,
}

impl Display for ChildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid => write!(formatter, "invalid"),
        }
    }
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

    #[test]
    fn validate_invalid_element_name() {
        let element = create_element("invalid", vec![], vec![]);

        assert_eq!(
            validate_element(&element),
            Err(ValidationError::InvalidTag("invalid".to_owned()))
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
                Err(ValidationError::InvalidElement {
                    attributes: [("invalid".into(), [AttributeError::Invalid].into())].into(),
                    children: Default::default(),
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
                validate_element(&element),
                Err(ValidationError::InvalidElement {
                    attributes: [
                        ("invalid-one".into(), [AttributeError::Invalid].into()),
                        ("invalid-two".into(), [AttributeError::Invalid].into()),
                    ]
                    .into(),
                    children: Default::default(),
                })
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
                Err(ValidationError::InvalidElement {
                    attributes: Default::default(),
                    children: [("div".into(), [ChildError::Invalid].into())].into(),
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
                validate_element(&element),
                Err(ValidationError::InvalidElement {
                    attributes: Default::default(),
                    children: [
                        ("div".into(), [ChildError::Invalid].into()),
                        ("table".into(), [ChildError::Invalid].into()),
                    ]
                    .into(),
                })
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
                Err(ValidationError::InvalidElement {
                    attributes: Default::default(),
                    children: [("p".into(), [ChildError::Invalid].into())].into(),
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
                validate_element(&element),
                Err(ValidationError::InvalidElement {
                    attributes: Default::default(),
                    children: [("div".into(), [ChildError::Invalid].into())].into(),
                })
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
                Err(ValidationError::InvalidElement {
                    attributes: Default::default(),
                    children: [("p".into(), [ChildError::Invalid].into())].into(),
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

            assert_eq!(validate_element(&element), Ok(()));
        }

        #[test]
        fn validate_invalid_child() {
            let element =
                create_element("table", vec![], vec![create_element("p", vec![], vec![])]);

            assert_eq!(
                validate_element(&element),
                Err(ValidationError::InvalidElement {
                    attributes: Default::default(),
                    children: [("p".into(), [ChildError::Invalid].into())].into(),
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

        #[test]
        fn validate_invalid_property() {
            let element = create_element("meta", vec![("property", "og:image")], vec![]);

            assert_eq!(
                validate_element(&element),
                Err(ValidationError::InvalidElement {
                    attributes: [("property".into(), [AttributeError::Invalid].into())].into(),
                    children: Default::default(),
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

            assert_eq!(validate_element(&element), Ok(()));
        }
    }
}
