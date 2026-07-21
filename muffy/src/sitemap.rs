mod error;

pub use self::error::SitemapError;
use core::str;
use quick_xml::{Reader, escape::unescape, events::Event};

const LOCATION_ELEMENT: &[u8] = b"loc";
const SITEMAP_ELEMENT: &[u8] = b"sitemap";
const URL_ELEMENT: &[u8] = b"url";

/// A sitemap location entry.
#[derive(Debug, Eq, PartialEq)]
pub enum Entry {
    /// A nested sitemap in a sitemap index.
    Sitemap(String),
    /// A page in a sitemap.
    Url(String),
}

/// Extracts locations from a sitemap or a sitemap index.
pub fn parse(source: &[u8]) -> Result<Vec<Entry>, SitemapError> {
    let mut reader = Reader::from_reader(source);
    reader.config_mut().trim_text(true);

    let mut buffer = vec![];
    let mut elements = Vec::<Vec<u8>>::new();
    let mut entries = vec![];
    let mut location = None;

    loop {
        match reader.read_event_into(&mut buffer)? {
            Event::Start(element) => {
                let name = element.local_name().as_ref().to_vec();

                if name == LOCATION_ELEMENT
                    && elements
                        .last()
                        .is_some_and(|parent| parent == SITEMAP_ELEMENT || parent == URL_ELEMENT)
                {
                    location = Some(String::new());
                }

                elements.push(name);
            }
            Event::Text(text) => {
                if let Some(location) = &mut location
                    && elements
                        .last()
                        .is_some_and(|element| element.as_slice() == LOCATION_ELEMENT)
                {
                    location.push_str(&unescape(&text.decode()?)?);
                }
            }
            Event::CData(data) => {
                if let Some(location) = &mut location
                    && elements
                        .last()
                        .is_some_and(|element| element.as_slice() == LOCATION_ELEMENT)
                {
                    location.push_str(str::from_utf8(&data.into_inner())?);
                }
            }
            Event::End(_) => {
                if elements.pop().as_deref() == Some(LOCATION_ELEMENT)
                    && let Some(location) = location.take()
                {
                    let location = location.trim();

                    if !location.is_empty() {
                        entries.push(
                            if elements
                                .last()
                                .is_some_and(|parent| parent.as_slice() == SITEMAP_ELEMENT)
                            {
                                Entry::Sitemap(location.to_owned())
                            } else {
                                Entry::Url(location.to_owned())
                            },
                        );
                    }
                }
            }
            Event::Eof => return Ok(entries),
            _ => {}
        }

        buffer.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parse_empty_document() {
        assert_eq!(parse(b"").unwrap(), vec![]);
    }

    #[test]
    fn parse_urlset() {
        assert_eq!(
            parse(
                br#"
                <urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                    <url><loc>https://foo.com/</loc></url>
                    <url><loc>https://foo.com/bar</loc></url>
                </urlset>
                "#
            )
            .unwrap(),
            vec![
                Entry::Url("https://foo.com/".into()),
                Entry::Url("https://foo.com/bar".into()),
            ]
        );
    }

    #[test]
    fn parse_sitemap_index() {
        assert_eq!(
            parse(
                br#"
                <sitemapindex xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
                    <sitemap><loc>https://foo.com/sitemap-0.xml</loc></sitemap>
                    <sitemap><loc>https://foo.com/sitemap-1.xml</loc></sitemap>
                </sitemapindex>
                "#
            )
            .unwrap(),
            vec![
                Entry::Sitemap("https://foo.com/sitemap-0.xml".into()),
                Entry::Sitemap("https://foo.com/sitemap-1.xml".into()),
            ]
        );
    }

    #[test]
    fn parse_with_xml_declaration() {
        assert_eq!(
            parse(
                br#"<?xml version="1.0" encoding="UTF-8"?>
                <urlset><url><loc>https://foo.com/</loc></url></urlset>"#
            )
            .unwrap(),
            vec![Entry::Url("https://foo.com/".into())]
        );
    }

    #[test]
    fn ignore_other_elements() {
        assert_eq!(
            parse(
                br#"
                <urlset>
                    <url>
                        <loc>https://foo.com/</loc>
                        <lastmod>1970-01-01</lastmod>
                        <changefreq>daily</changefreq>
                        <priority>1</priority>
                    </url>
                </urlset>
                "#
            )
            .unwrap(),
            vec![Entry::Url("https://foo.com/".into())]
        );
    }

    #[test]
    fn ignore_invalid_change_frequency() {
        assert_eq!(
            parse(
                br#"
                <urlset>
                    <url>
                        <loc>https://foo.com/</loc>
                        <changefreq>whenever</changefreq>
                    </url>
                </urlset>
                "#
            )
            .unwrap(),
            vec![Entry::Url("https://foo.com/".into())]
        );
    }

    #[test]
    fn trim_location_whitespace() {
        assert_eq!(
            parse(b"<urlset><url><loc>\n  https://foo.com/  \n</loc></url></urlset>").unwrap(),
            vec![Entry::Url("https://foo.com/".into())]
        );
    }

    #[test]
    fn unescape_location_entities() {
        assert_eq!(
            parse(b"<urlset><url><loc>https://foo.com/?a=1&amp;b=2</loc></url></urlset>").unwrap(),
            vec![Entry::Url("https://foo.com/?a=1&b=2".into())]
        );
    }

    #[test]
    fn unescape_numeric_character_reference() {
        assert_eq!(
            parse(b"<urlset><url><loc>https://foo.com/a&#47;b</loc></url></urlset>").unwrap(),
            vec![Entry::Url("https://foo.com/a/b".into())]
        );
    }

    #[test]
    fn parse_location_in_cdata() {
        assert_eq!(
            parse(b"<urlset><url><loc><![CDATA[https://foo.com/]]></loc></url></urlset>").unwrap(),
            vec![Entry::Url("https://foo.com/".into())]
        );
    }

    #[test]
    fn ignore_nested_element_in_location() {
        assert_eq!(
            parse(b"<urlset><url><loc>https://foo.com/<em>x</em></loc></url></urlset>").unwrap(),
            vec![Entry::Url("https://foo.com/".into())]
        );
    }

    #[test]
    fn skip_empty_location() {
        assert_eq!(
            parse(b"<urlset><url><loc></loc></url></urlset>").unwrap(),
            vec![]
        );
    }

    #[test]
    fn ignore_location_outside_entry() {
        assert_eq!(
            parse(b"<urlset><loc>https://foo.com/</loc></urlset>").unwrap(),
            vec![]
        );
    }

    #[test]
    fn keep_locations_before_truncation() {
        assert_eq!(
            parse(b"<urlset><url><loc>https://foo.com/</loc></url>").unwrap(),
            vec![Entry::Url("https://foo.com/".into())]
        );
    }

    #[test]
    fn fail_on_mismatched_tags() {
        assert!(parse(b"<urlset><url><loc>https://foo.com/</wrong></url></urlset>").is_err());
    }

    #[test]
    fn fail_on_invalid_entity() {
        assert!(parse(b"<urlset><url><loc>https://foo.com/?a&b</loc></url></urlset>").is_err());
    }

    #[test]
    fn fail_on_invalid_utf8_in_text() {
        assert!(parse(b"<urlset><url><loc>\xff</loc></url></urlset>").is_err());
    }

    #[test]
    fn fail_on_invalid_utf8_in_cdata() {
        assert!(parse(b"<urlset><url><loc><![CDATA[\xff]]></loc></url></urlset>").is_err());
    }
}
