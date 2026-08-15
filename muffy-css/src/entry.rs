/// A URL entry in a style sheet.
#[derive(Debug, Eq, PartialEq)]
pub enum Entry {
    /// An imported style sheet.
    Import(String),
    /// A URL referenced in a rule.
    Url(String),
}
