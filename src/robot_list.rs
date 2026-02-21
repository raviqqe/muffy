use core::ops::Deref;
use robotxt::Robots;
use url::Url;

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

    pub fn is_allowed(&self, url: &Url) -> bool {
        self.db.is_absolute_allowed(url)
    }

    pub fn sitemaps(&self) -> impl Iterator<Item = &str> {
        self.sitemaps.iter().map(Deref::deref)
    }
}
