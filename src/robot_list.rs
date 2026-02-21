use core::ops::Deref;
use robotstxt_rs::RobotsTxt;

const USER_AGENT: &str = "MuffyBot";

pub struct RobotList {
    db: RobotsTxt,
}

impl RobotList {
    pub fn parse(source: &str) -> Self {
        Self {
            db: RobotsTxt::parse(source),
        }
    }

    pub fn is_allowed(&self, path: &str) -> bool {
        self.db.can_fetch(USER_AGENT, path)
    }

    pub fn sitemaps(&self) -> impl Iterator<Item = &str> {
        self.db.get_sitemaps().iter().map(Deref::deref)
    }
}
