//! Document validation.

use muffy_document::html::Element;
use muffy_validation_macro::html;

html! {}

/// A validation error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ValidationError {
    /// An invalid attribute.
    InvalidAttribute(String),
    /// An invalid child.
    InvalidChild(String),
    /// An invalid element.
    InvalidElement(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use muffy_document::html::Node;
    use std::sync::Arc;

    #[test]
    fn validate_valid_html_element() {
        let element = Element::new("html".to_owned(), vec![], vec![]);

        assert_eq!(validate_element(&element), Ok(()));
    }

    #[test]
    fn validate_invalid_element_name() {
        let element = Element::new("invalid".to_owned(), vec![], vec![]);

        assert_eq!(
            validate_element(&element),
            Err(ValidationError::InvalidElement("invalid".to_owned()))
        );
    }

    #[test]
    fn validate_valid_attribute() {
        let element = Element::new(
            "div".to_owned(),
            vec![("id".to_owned(), "foo".to_owned())],
            vec![],
        );

        assert_eq!(validate_element(&element), Ok(()));
    }

    #[test]
    fn validate_invalid_attribute() {
        let element = Element::new(
            "div".to_owned(),
            vec![("invalid".to_owned(), "bar".to_owned())],
            vec![],
        );

        assert_eq!(
            validate_element(&element),
            Err(ValidationError::InvalidAttribute("invalid".to_owned()))
        );
    }

    #[test]
    fn validate_valid_child() {
        let element = Element::new(
            "div".to_owned(),
            vec![],
            vec![Arc::new(Node::Element(Element::new(
                "p".to_owned(),
                vec![],
                vec![],
            )))],
        );

        assert_eq!(validate_element(&element), Ok(()));
    }

    #[test]
    fn validate_invalid_child() {
        let element = Element::new(
            "p".to_owned(),
            vec![],
            vec![Arc::new(Node::Element(Element::new(
                "div".to_owned(),
                vec![],
                vec![],
            )))],
        );

        assert_eq!(
            validate_element(&element),
            Err(ValidationError::InvalidChild("div".to_owned()))
        );
    }
}
