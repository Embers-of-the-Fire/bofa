pub mod sensitive;

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
pub struct ScannerConfig {
    #[serde(default)]
    pub sensitive: sensitive::SensitiveScannerConfig,
}
