use robotxt::Robots;
use url::Url;

const USER_AGENT: &str = "Muffy";

pub struct RobotList {
    db: Robots,
}

impl RobotList {
    pub fn parse(source: &str) -> Self {
        Self {
            db: Robots::from_bytes(source.as_bytes(), USER_AGENT),
        }
    }

    pub fn is_allowed(&self, url: &Url) -> bool {
        self.db.is_absolute_allowed(url)
    }

    pub fn sitemaps(&self) -> impl Iterator<Item = &url::Url> {
        self.db.sitemaps().iter()
    }
}
