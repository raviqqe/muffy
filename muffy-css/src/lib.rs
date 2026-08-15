//! CSS documents.

extern crate alloc;

mod entry;
mod error;

pub use self::{entry::Entry, error::CssError};
use alloc::sync::Arc;
use core::{convert::Infallible, str};
use itertools::Itertools;
use lightningcss::{
    error::{Error, ParserError},
    rules::CssRule,
    stylesheet::{ParserOptions, StyleSheet},
    values::url::Url,
    visit_types,
    visitor::{Visit, VisitTypes, Visitor},
};
use std::sync::RwLock;

/// Extracts URL entries from a style sheet together with syntax error
/// messages.
pub fn parse(source: &[u8]) -> Result<(Vec<Entry>, Vec<String>), CssError> {
    let source = str::from_utf8(source)?;
    let warnings = Arc::new(RwLock::new(vec![]));
    let mut stylesheet = StyleSheet::parse(
        // cspell: disable-next-line
        source.strip_prefix('\u{feff}').unwrap_or(source),
        ParserOptions {
            error_recovery: true,
            warnings: Some(warnings.clone()),
            ..Default::default()
        },
    )
    .map_err(|error| CssError::Syntax(format_error(&error)))?;
    let mut visitor = UrlVisitor::default();

    stylesheet.visit(&mut visitor)?;

    Ok((
        visitor
            .entries
            .into_iter()
            .filter(|entry| {
                let (Entry::Import(url) | Entry::Url(url)) = entry;

                // Fragment-only URLs refer to elements in referencing documents
                // rather than in style sheets themselves.
                !url.is_empty() && !url.starts_with('#')
            })
            .collect(),
        warnings
            .read()?
            .iter()
            .map(format_error)
            .unique()
            .sorted()
            .collect(),
    ))
}

fn format_error(error: &Error<ParserError<'_>>) -> String {
    if let Some(location) = &error.loc {
        format!(
            "{} at {}:{}",
            error.kind,
            location.line + 1,
            location.column
        )
    } else {
        error.kind.to_string()
    }
}

#[derive(Default)]
struct UrlVisitor {
    entries: Vec<Entry>,
}

impl<'a> Visitor<'a> for UrlVisitor {
    type Error = Infallible;

    fn visit_types(&self) -> VisitTypes {
        visit_types!(URLS | RULES | RESOLUTIONS)
    }

    fn visit_rule(&mut self, rule: &mut CssRule<'a>) -> Result<(), Self::Error> {
        if let CssRule::Import(import) = rule {
            self.entries.push(Entry::Import(import.url.to_string()));
        }

        rule.visit_children(self)
    }

    fn visit_url(&mut self, url: &mut Url<'a>) -> Result<(), Self::Error> {
        self.entries.push(Entry::Url(url.url.to_string()));

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn parse_entries(source: &[u8]) -> Vec<Entry> {
        let (entries, errors) = parse(source).unwrap();

        assert_eq!(errors, Vec::<String>::new());

        entries
    }

    #[test]
    fn parse_empty_document() {
        assert_eq!(parse_entries(b""), vec![]);
    }

    #[test]
    fn parse_import() {
        assert_eq!(
            parse_entries(br#"@import "foo.css";"#),
            vec![Entry::Import("foo.css".into())]
        );
    }

    #[test]
    fn parse_import_with_url_function() {
        assert_eq!(
            parse_entries(br#"@import url("foo.css");"#),
            vec![Entry::Import("foo.css".into())]
        );
    }

    #[test]
    fn parse_import_with_media_query() {
        assert_eq!(
            parse_entries(br#"@import "foo.css" screen;"#),
            vec![Entry::Import("foo.css".into())]
        );
    }

    #[test]
    fn parse_import_after_byte_order_mark() {
        assert_eq!(
            // cspell: disable-next-line
            parse_entries("\u{feff}@import \"foo.css\";".as_bytes()),
            vec![Entry::Import("foo.css".into())]
        );
    }

    #[test]
    fn parse_url_in_property() {
        assert_eq!(
            parse_entries(b"a { background: url(foo.png); }"),
            vec![Entry::Url("foo.png".into())]
        );
    }

    #[test]
    fn parse_url_in_font_face_rule() {
        assert_eq!(
            parse_entries(br#"@font-face { src: url("foo.woff2") format("woff2"); }"#),
            vec![Entry::Url("foo.woff2".into())]
        );
    }

    #[test]
    fn parse_url_in_media_rule() {
        assert_eq!(
            parse_entries(b"@media screen { a { background: url(foo.png); } }"),
            vec![Entry::Url("foo.png".into())]
        );
    }

    #[test]
    fn parse_urls_in_image_set() {
        assert_eq!(
            parse_entries(br#"a { background: image-set(url("foo.png") 1x, "bar.png" 2x); }"#),
            vec![Entry::Url("foo.png".into()), Entry::Url("bar.png".into())]
        );
    }

    #[test]
    fn parse_url_in_unknown_property() {
        assert_eq!(
            parse_entries(b"a { behavior: url(foo.htc); }"),
            vec![Entry::Url("foo.htc".into())]
        );
    }

    #[test]
    fn parse_multiple_urls() {
        assert_eq!(
            parse_entries(
                br#"
                @import "foo.css";

                a {
                    background: url(bar.png);
                    cursor: url(baz.png);
                }
                "#
            ),
            vec![
                Entry::Import("foo.css".into()),
                Entry::Url("bar.png".into()),
                Entry::Url("baz.png".into()),
            ]
        );
    }

    #[test]
    fn parse_data_url() {
        assert_eq!(
            parse_entries(br#"a { background: url("data:image/svg+xml,<svg/>"); }"#),
            vec![Entry::Url("data:image/svg+xml,<svg/>".into())]
        );
    }

    #[test]
    fn skip_empty_url() {
        assert_eq!(parse_entries(b"a { background: url(); }"), vec![]);
    }

    #[test]
    fn skip_fragment_url() {
        assert_eq!(parse_entries(b"a { filter: url(#foo); }"), vec![]);
    }

    #[test]
    fn parse_rule_unclosed_at_end_of_input() {
        assert_eq!(parse_entries(b"a {"), vec![]);
    }

    #[test]
    fn report_syntax_error() {
        let (entries, errors) = parse(b"} a { background: url(foo.png); }").unwrap();

        assert_eq!(entries, vec![]);
        assert_eq!(errors, vec!["Invalid empty selector at 1:1".to_owned()]);
    }

    #[test]
    fn report_syntax_error_with_recovered_url() {
        let (entries, errors) =
            parse(b"@unknown-rule { x } a { background: url(foo.png); }").unwrap();

        assert_eq!(entries, vec![Entry::Url("foo.png".into())]);
        assert_eq!(
            errors,
            vec!["Unknown at rule: @unknown-rule at 1:14".to_owned()]
        );
    }

    #[test]
    fn report_multiple_syntax_errors() {
        let (entries, errors) = parse(b"@unknown-first { x } @unknown-second { x }").unwrap();

        assert_eq!(entries, vec![]);
        assert_eq!(
            errors,
            vec![
                "Unknown at rule: @unknown-first at 1:15".to_owned(),
                "Unknown at rule: @unknown-second at 1:37".to_owned(),
            ]
        );
    }

    #[test]
    fn report_misplaced_import() {
        let (entries, errors) = parse(br#"a { color: red; } @import "foo.css";"#).unwrap();

        assert_eq!(entries, vec![]);
        assert_eq!(
            errors,
            vec![
                "@import rules must precede all rules aside from @charset and @layer statements \
                 at 1:26"
                    .to_owned()
            ]
        );
    }

    #[test]
    fn report_utf8_error() {
        let error = parse(b"/* caf\xe9 */ a { background: url(foo.png); }").unwrap_err();

        assert!(matches!(error, CssError::Utf8(_)));
        assert_eq!(
            error.to_string(),
            "invalid utf-8 sequence of 1 bytes from index 6"
        );
    }
}
