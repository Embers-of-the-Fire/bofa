use super::Provider;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct WorkerConfig {
    #[serde(default)]
    pub provider: Provider,
    #[serde(default)]
    pub dry_run: bool,
    #[serde(default = "default_true")]
    pub post_comments: bool,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            provider: Default::default(),
            dry_run: false,
            post_comments: true,
        }
    }
}

fn default_true() -> bool {
    true
}
