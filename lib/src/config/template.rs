use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
pub struct TemplateConfig {
    #[serde(default)]
    pub scanner: ScannerTemplateConfig,
    #[serde(default)]
    pub comment: CommentTemplateConfig,
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
pub struct CommentTemplateConfig {
    #[serde(default)]
    pub footnote: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
pub struct ScannerTemplateConfig {
    #[serde(default)]
    pub sensitive: SensitiveTemplateConfig,
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
pub struct SensitiveTemplateConfig {
    #[serde(default)]
    pub report: Option<String>,
    #[serde(default)]
    pub empty_report: Option<String>,
}
