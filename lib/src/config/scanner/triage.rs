use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct TriageConfig {
    pub enabled: bool,
    #[serde(default)]
    pub post_comment: bool,
    #[serde(default)]
    pub groups: IndexMap<String, TriageGroup>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct TriageGroup {
    pub description: String,
    pub paths: Vec<String>,
    pub labels: Vec<String>,
}
