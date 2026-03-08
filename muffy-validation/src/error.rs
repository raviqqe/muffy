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
    },
}

impl Display for MarkupError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTag(tag) => write!(formatter, "unknown tag \"{tag}\""),
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

/// An attribute validation error.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum AttributeError {
    /// Not allowed.
    NotAllowed,
}

impl Display for AttributeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAllowed => write!(formatter, "not allowed"),
        }
    }
}

/// A child validation error.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ChildError {
    /// Not allowed.
    NotAllowed,
}

impl Display for ChildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
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
                }
            ),
            "invalid attributes: foo (not allowed)"
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
                }
            ),
            "invalid children: foo (not allowed)"
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
                }
            ),
            "invalid attributes: foo (not allowed), invalid children: bar (not allowed)"
        );
    }
}
