use crate::action::comment_marker::CommentMarker;
use crate::config::repository::RepositoryConfig;
use crate::templates::{COMMENT_FOOTNOTE_TEMPLATE, TRIAGE_PR_TEMPLATE};
use tera::{Context, Tera};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrInput {
    pub owner: String,
    pub repo: String,
    pub id: u64,
}

impl PrInput {
    pub fn from_repository(id: u64, repository: &RepositoryConfig) -> Self {
        Self {
            owner: repository.owner.clone(),
            repo: repository.repo.clone(),
            id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentStatus {
    Created,
    Updated,
    Unchanged,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TriagePrOutput {
    pub body: Option<String>,
    pub status: CommentStatus,
    pub comment_url: Option<String>,
    pub labels_applied: Vec<String>,
    pub labels_missing: Vec<String>,
}

pub const COMMENT_MARKER: &str = "<!-- bofa:triage-pr -->";
pub const TRIAGE_COMMENT_MARKER: CommentMarker = CommentMarker::new(COMMENT_MARKER);

#[derive(Debug, Clone)]
pub struct PrTriageResult {
    pub findings: Vec<crate::scanner::triage::TriageFinding>,
    pub footnote_template: Option<String>,
    pub app_name: Option<String>,
}

impl PrTriageResult {
    pub fn render(&self) -> Result<Option<String>, crate::action::triage::Error> {
        if self.findings.is_empty() {
            return Ok(None);
        }

        let mut context = Context::new();
        context.insert("findings", &self.findings);
        let rendered = Tera::one_off(TRIAGE_PR_TEMPLATE, &context, false)
            .map_err(|err| crate::action::triage::Error::Template(err.to_string()))?;
        Ok(Some(self.append_footnote(rendered.trim_end().to_string())?))
    }

    fn append_footnote(&self, body: String) -> Result<String, crate::action::triage::Error> {
        let template = match self.footnote_template.as_deref() {
            Some("") => return Ok(body),
            Some(template) => template,
            None => COMMENT_FOOTNOTE_TEMPLATE,
        };
        let mut context = Context::new();
        context.insert("app_name", &self.app_name);
        let rendered = Tera::one_off(template, &context, false)
            .map_err(|err| crate::action::triage::Error::Template(err.to_string()))?;
        Ok(format!("{body}\n\n{}", rendered.trim_end()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner::triage::TriageFinding;

    #[test]
    fn builds_from_repository() {
        let repository = RepositoryConfig {
            owner: "alice".to_string(),
            repo: "repo".to_string(),
        };
        let input = PrInput::from_repository(42, &repository);
        assert_eq!(input.owner, "alice");
        assert_eq!(input.repo, "repo");
        assert_eq!(input.id, 42);
    }

    #[test]
    fn renders_no_findings_as_none() {
        let result = PrTriageResult {
            findings: Vec::new(),
            footnote_template: Some(String::new()),
            app_name: None,
        };
        assert!(result.render().unwrap().is_none());
    }

    #[test]
    fn renders_triage_output() {
        let findings = vec![
            TriageFinding {
                name: "core-repo".to_string(),
                description: "Core repo".to_string(),
                matched_paths: vec!["src/main.rs".to_string()],
                labels: vec!["core-impact".to_string()],
            },
            TriageFinding {
                name: "docs".to_string(),
                description: "Documentation".to_string(),
                matched_paths: vec!["README.md".to_string()],
                labels: vec!["docs".to_string()],
            },
        ];
        let result = PrTriageResult {
            findings,
            footnote_template: Some(String::new()),
            app_name: None,
        };
        let expected = r#"Automatically triage the pull request to:
- core-repo: Core repo
- docs: Documentation"#;
        assert_eq!(result.render().unwrap(), Some(expected.to_string()));
    }

    #[test]
    fn appends_default_footnote_with_app_name() {
        let result = PrTriageResult {
            findings: vec![TriageFinding {
                name: "core".to_string(),
                description: "Core".to_string(),
                matched_paths: vec!["src/main.rs".to_string()],
                labels: vec!["core".to_string()],
            }],
            footnote_template: None,
            app_name: Some("bofa-app".to_string()),
        };
        let rendered = result.render().unwrap().unwrap();
        assert!(rendered.starts_with("Automatically triage the pull request to:"));
        assert!(rendered.contains("commented by @bofa-app"));
    }

    #[test]
    fn renders_custom_footnote_template() {
        let result = PrTriageResult {
            findings: vec![TriageFinding {
                name: "core".to_string(),
                description: "Core".to_string(),
                matched_paths: vec!["src/main.rs".to_string()],
                labels: vec!["core".to_string()],
            }],
            footnote_template: Some("powered by {{ app_name }}".to_string()),
            app_name: Some("bofa-app".to_string()),
        };
        assert_eq!(
            result.render().unwrap(),
            Some(
                "Automatically triage the pull request to:\n- core: Core\n\npowered by bofa-app"
                    .to_string()
            )
        );
    }
}
