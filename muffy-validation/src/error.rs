use alloc::collections::{BTreeMap, BTreeSet};
use core::fmt::{self, Display, Formatter};

/// A markup error.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MarkupError {
    /// An unknown tag.
    UnknownTag(String),
    /// Invalid element.
    InvalidElement {
        /// Invalid attributes.
        attributes: BTreeMap<String, BTreeSet<AttributeError>>,
        /// Invalid children.
        children: BTreeMap<String, BTreeSet<ChildError>>,
        /// Missing required attributes.
        missing_attributes: BTreeSet<String>,
        /// Missing required children.
        missing_children: BTreeSet<String>,
    },
}

impl Display for MarkupError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTag(tag) => write!(formatter, "unknown tag \"{tag}\""),
            Self::InvalidElement {
                attributes,
                children,
                missing_attributes,
                missing_children,
            } => write!(
                formatter,
                "{}",
                [
                    format_errors("invalid attributes", attributes),
                    format_errors("invalid children", children),
                    format_names("missing attributes", missing_attributes),
                    format_names("missing children", missing_children),
                ]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(", ")
            ),
        }
    }
}

fn format_errors<E: Display>(
    label: &str,
    errors: &BTreeMap<String, BTreeSet<E>>,
) -> Option<String> {
    (!errors.is_empty()).then(|| {
        format!(
            "{label}: {}",
            errors
                .iter()
                .map(|(name, errors)| format!(
                    "{name} ({})",
                    errors
                        .iter()
                        .map(ToString::to_string)
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
                .collect::<Vec<_>>()
                .join(", ")
        )
    })
}

fn format_names(label: &str, names: &BTreeSet<String>) -> Option<String> {
    (!names.is_empty()).then(|| {
        format!(
            "{label}: {}",
            names.iter().cloned().collect::<Vec<_>>().join(", ")
        )
    })
}

/// An attribute markup error.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AttributeError {
    /// Conflicting with other attributes.
    Conflicting,
    /// Not allowed.
    NotAllowed,
}

impl Display for AttributeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflicting => write!(formatter, "conflicting"),
            Self::NotAllowed => write!(formatter, "not allowed"),
        }
    }
}

/// A child markup error.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ChildError {
    /// Misplaced.
    Misplaced,
    /// Not allowed.
    NotAllowed,
}

impl Display for ChildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Misplaced => write!(formatter, "misplaced"),
            Self::NotAllowed => write!(formatter, "not allowed"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_unknown_tag() {
        assert_eq!(
            format!("{}", MarkupError::UnknownTag("foo".into())),
            "unknown tag \"foo\""
        );
    }

    #[test]
    fn display_not_allowed_attributes() {
        assert_eq!(
            format!(
                "{}",
                MarkupError::InvalidElement {
                    attributes: [("foo".into(), [AttributeError::NotAllowed].into())].into(),
                    children: Default::default(),
                    missing_attributes: Default::default(),
                    missing_children: Default::default(),
                }
            ),
            "invalid attributes: foo (not allowed)"
        );
    }

    #[test]
    fn display_conflicting_attribute() {
        assert_eq!(
            format!(
                "{}",
                MarkupError::InvalidElement {
                    attributes: [("foo".into(), [AttributeError::Conflicting].into())].into(),
                    children: Default::default(),
                    missing_attributes: Default::default(),
                    missing_children: Default::default(),
                }
            ),
            "invalid attributes: foo (conflicting)"
        );
    }

    #[test]
    fn display_not_allowed_children() {
        assert_eq!(
            format!(
                "{}",
                MarkupError::InvalidElement {
                    attributes: Default::default(),
                    children: [("foo".into(), [ChildError::NotAllowed].into())].into(),
                    missing_attributes: Default::default(),
                    missing_children: Default::default(),
                }
            ),
            "invalid children: foo (not allowed)"
        );
    }

    #[test]
    fn display_misplaced_child() {
        assert_eq!(
            format!(
                "{}",
                MarkupError::InvalidElement {
                    attributes: Default::default(),
                    children: [("foo".into(), [ChildError::Misplaced].into())].into(),
                    missing_attributes: Default::default(),
                    missing_children: Default::default(),
                }
            ),
            "invalid children: foo (misplaced)"
        );
    }

    #[test]
    fn display_missing_attributes() {
        assert_eq!(
            format!(
                "{}",
                MarkupError::InvalidElement {
                    attributes: Default::default(),
                    children: Default::default(),
                    missing_attributes: ["bar".into(), "foo".into()].into(),
                    missing_children: Default::default(),
                }
            ),
            "missing attributes: bar, foo"
        );
    }

    #[test]
    fn display_missing_children() {
        assert_eq!(
            format!(
                "{}",
                MarkupError::InvalidElement {
                    attributes: Default::default(),
                    children: Default::default(),
                    missing_attributes: Default::default(),
                    missing_children: ["title".into()].into(),
                }
            ),
            "missing children: title"
        );
    }

    #[test]
    fn display_not_allowed_attributes_and_children() {
        assert_eq!(
            format!(
                "{}",
                MarkupError::InvalidElement {
                    attributes: [("foo".into(), [AttributeError::NotAllowed].into())].into(),
                    children: [("bar".into(), [ChildError::NotAllowed].into())].into(),
                    missing_attributes: Default::default(),
                    missing_children: Default::default(),
                }
            ),
            "invalid attributes: foo (not allowed), invalid children: bar (not allowed)"
        );
    }
}
