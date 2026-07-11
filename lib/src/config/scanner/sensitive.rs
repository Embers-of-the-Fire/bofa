use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct SensitiveScannerConfig {
    pub enabled: bool,
    pub item: Vec<SensitiveScannerItem>,
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
pub struct SensitiveScannerItem {
    pub description: String,
    pub paths: Vec<String>,
    #[serde(default)]
    pub members: Vec<String>,
}
