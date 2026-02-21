use robotparser::{model::RobotsTxt, parser::parse_robots_txt, service::RobotsTxtService};

const USER_AGENT: &str = "MuffyBot";

pub struct RobotList {
    db: RobotsTxt,
}

impl RobotList {
    pub fn parse(source: &str) -> Self {
        Self {
            db: parse_robots_txt("localhost", source),
        }
    }

    pub fn is_allowed(&self, path: &str) -> bool {
        self.db.can_fetch(USER_AGENT, path)
    }

    pub fn sitemaps(&self) -> impl Iterator<Item = &str> {
        self.db.get_sitemaps().iter().map(|url| url.as_str())
    }
}
