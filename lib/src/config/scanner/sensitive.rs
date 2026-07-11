use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct SensitiveScannerConfig {
    pub enabled: bool,
    pub item: Vec<SensitiveScannerItem>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct SensitiveScannerItem {
    pub description: String,
    pub paths: Vec<String>,
    #[serde(default)]
    pub members: Vec<String>,
}
