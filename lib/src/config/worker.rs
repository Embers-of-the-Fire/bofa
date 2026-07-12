use super::Provider;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
pub struct WorkerConfig {
    #[serde(default)]
    pub provider: Provider,
    #[serde(default)]
    pub dry_run: bool,
}
