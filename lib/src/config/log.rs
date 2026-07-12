use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    #[default]
    Full,
    Compact,
    Pretty,
    Json,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct LogConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub format: Format,
    #[serde(default = "default_level")]
    pub level: String,
}

impl Default for LogConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            format: Format::default(),
            level: default_level(),
        }
    }
}

fn default_enabled() -> bool {
    true
}

fn default_level() -> String {
    "warn".to_string()
}
