use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct SensitiveScannerConfig {
    pub enabled: bool,
    #[serde(default)]
    pub always_report: bool,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub groups: IndexMap<String, SensitiveScannerItem>,
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
pub struct SensitiveScannerItem {
    pub description: String,
    pub paths: Vec<String>,
    #[serde(default)]
    pub members: Vec<String>,
    #[serde(default)]
    pub labels: Vec<String>,
}
