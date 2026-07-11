pub mod credentials;
pub mod scanner;

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct BofaConfig {
    pub credentials: credentials::Credentials,
    #[serde(default)]
    pub scanner: scanner::ScannerConfig,
}

pub fn load_config(path: impl AsRef<Path>) -> Result<BofaConfig, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(path)?;
    let config: BofaConfig = toml::from_str(&contents)?;
    Ok(config)
}
