use robotxt::Robots;

const USER_AGENT: &str = "MuffyBot";

pub struct RobotList {
    db: Robots,
}

impl RobotList {
    pub fn parse(source: &str) -> Self {
        Self {
            db: Robots::from_bytes(source.as_bytes(), USER_AGENT),
        }
    }

    pub fn is_allowed(&self, path: &str) -> bool {
        self.db.is_relative_allowed(path)
    }

    pub fn sitemaps(&self) -> impl Iterator<Item = &str> {
        self.db.sitemaps().iter().map(|url| url.as_str())
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
            user-agent: *
            sitemap: https://example.com/primary.xml\r
            sitemap: https://example.com/secondary.xml
            Sitemap: https://example.com/tertiary.xml
        "};
        let list = RobotList::parse(source);

        assert_eq!(
            list.sitemaps().collect::<Vec<_>>(),
            vec![
                "https://example.com/primary.xml",
                "https://example.com/secondary.xml",
                "https://example.com/tertiary.xml",
            ]
        );
    }

    #[test]
    fn parse_ignores_non_matching_sitemap_lines() {
        let source = indoc! {"
            sitemap:https://example.com/no-space.xml
             sitemap: https://example.com/leading-space.xml
            SITEMAP: https://example.com/upper.xml
            Sitemap: https://example.com/title.xml
        "};
        let list = RobotList::parse(source);

        assert_eq!(
            list.sitemaps().collect::<Vec<_>>(),
            vec![
                "https://example.com/no-space.xml",
                "https://example.com/leading-space.xml",
                "https://example.com/upper.xml",
                "https://example.com/title.xml"
            ]
        );
    }

    #[test]
    fn is_allowed_respects_disallow() {
        let source = indoc! {"
            user-agent: MuffyBot
            disallow: /private
        "};
        let list = RobotList::parse(source);

        assert!(list.is_allowed("/public"));
        assert!(!list.is_allowed("/private"));
    }

    #[test]
    fn allow_paths() {
        let source = indoc! {"
            user-agent: MuffyBot
            disallow: /private
            allow: /private/public
        "};
        let list = RobotList::parse(source);

        assert!(list.is_allowed("/private/public"));
        assert!(!list.is_allowed("/private/secret"));
    }
}
