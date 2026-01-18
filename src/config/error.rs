use core::fmt;
use core::fmt::Display;
use core::fmt::Formatter;

pub enum ConfigError {
    ExcludeSite(String),
}

impl Display for ConfigError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::ExcludeSite(site) => {
                write!(formatter, "The site '{}' is marked for exclusion.", site)
            }
        }
    }
}

impl Error for ConfigError {}
