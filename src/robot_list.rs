use core::ops::Deref;
use robotxt::Robots;

const USER_AGENT: &str = "MuffyBot";

pub struct RobotList {
    db: Robots,
    sitemaps: Vec<String>,
}

impl RobotList {
    pub fn parse(source: &str) -> Self {
        Self {
            db: Robots::from_bytes(source.as_bytes(), USER_AGENT),
            sitemaps: source
                .split("\n")
                .filter(|line| line.starts_with("sitemap:") || line.starts_with("Sitemap: "))
                .map(|line| {
                    line.trim_start_matches("sitemap:")
                        .trim_start_matches("Sitemap:")
                        .trim()
                        .to_owned()
                })
                .collect(),
        }
    }

    pub fn is_allowed(&self, path: &str) -> bool {
        self.db.is_relative_allowed(path)
    }

    pub fn sitemaps(&self) -> impl Iterator<Item = &str> {
        self.sitemaps.iter().map(Deref::deref)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use pretty_assertions::assert_eq;

    #[test]
    fn parse_sitemaps_with_trimming() {
        let source = indoc! {"
            User-agent: *
            Sitemap: https://example.com/sitemap.xml\r
            sitemap:https://example.com/secondary.xml
            Sitemap:    https://example.com/tertiary.xml
        "};
        let list = RobotList::parse(source);

        assert_eq!(
            list.sitemaps().collect::<Vec<_>>(),
            vec![
                "https://example.com/sitemap.xml",
                "https://example.com/secondary.xml",
                "https://example.com/tertiary.xml",
            ]
        );
    }

    #[test]
    fn parse_ignores_non_matching_sitemap_lines() {
        let source = indoc! {"
            Sitemap:https://example.com/no-space.xml
             sitemap: https://example.com/leading-space.xml
            SITEMAP: https://example.com/uppercase.xml
            Sitemap: https://example.com/valid.xml
        "};
        let list = RobotList::parse(source);

        assert_eq!(
            list.sitemaps().collect::<Vec<_>>(),
            vec!["https://example.com/valid.xml"]
        );
    }

    #[test]
    fn is_allowed_respects_disallow() {
        let source = indoc! {"
            User-agent: MuffyBot
            Disallow: /private
        "};
        let list = RobotList::parse(source);

        assert!(list.is_allowed("/public"));
        assert!(!list.is_allowed("/private"));
    }

    #[test]
    fn allow_paths() {
        let source = indoc! {"
            User-agent: MuffyBot
            Disallow: /private
            Allow: /private/public
        "};
        let list = RobotList::parse(source);

        assert!(list.is_allowed("/private/public"));
        assert!(!list.is_allowed("/private/secret"));
    }
}
