pub mod credentials;
pub mod repository;
pub mod scanner;

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    #[default]
    GitHub,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct BofaConfig {
    #[serde(default)]
    pub provider: Provider,
    pub credentials: credentials::Credentials,
    pub repository: repository::RepositoryConfig,
    #[serde(default)]
    pub scanner: scanner::ScannerConfig,
}

pub fn load_config(path: impl AsRef<Path>) -> Result<BofaConfig, Box<dyn std::error::Error>> {
    let contents = fs::read_to_string(path)?;
    let config: BofaConfig = toml::from_str(&contents)?;
    config.repository.validate()?;
    Ok(config)
}
