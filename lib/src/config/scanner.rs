pub mod sensitive;
pub mod triage;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ScannerConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub sensitive: sensitive::SensitiveScannerConfig,
    #[serde(default)]
    pub triage: triage::TriageConfig,
}

impl Default for ScannerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            sensitive: Default::default(),
            triage: Default::default(),
        }
    }
}

fn default_true() -> bool {
    true
}
