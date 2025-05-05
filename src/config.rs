use serde::Deserialize;
use std::collections::HashMap;

/// A validation configuration.
#[derive(Debug, Deserialize)]
pub struct Config {
    origins: HashMap<String, String>,
}

impl Config {}
