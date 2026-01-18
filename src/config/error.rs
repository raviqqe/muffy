use core::error::Error;
use core::fmt;
use core::fmt::Display;
use core::fmt::Formatter;

pub enum ConfigError {
    ExcludeSite(String),
}

impl Display for ConfigError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::ExcludeSite(url) => {
                write!(formatter, "exclude field must be true if present: {url}")
            }
        }
    }
}

impl Error for ConfigError {}
