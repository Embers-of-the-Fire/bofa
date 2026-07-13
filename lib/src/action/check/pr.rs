use crate::config::repository::RepositoryConfig;
use crate::git::PullRequestMetadata;
use crate::scanner::sensitive::SensitiveFinding;
use crate::templates::{CHECK_PR_EMPTY_TEMPLATE, CHECK_PR_TEMPLATE};
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

pub fn format_pr_metadata(metadata: &PullRequestMetadata) -> String {
    let draft = if metadata.draft { " [draft]" } else { "" };
    format!(
        "#{} {} by {} [{}]{} {}",
        metadata.number, metadata.title, metadata.author, metadata.state, draft, metadata.url
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckPrOutput {
    pub body: Option<String>,
    pub posted: bool,
    pub comment_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PrCheckResult {
    pub metadata: PullRequestMetadata,
    pub findings: Vec<SensitiveFinding>,
    pub scanner_enabled: bool,
    pub always_report: bool,
    pub report_template: Option<String>,
    pub empty_report_template: Option<String>,
}

impl PrCheckResult {
    pub fn render(&self) -> Result<Option<String>, crate::action::check::Error> {
        let (template, trim) = if !self.findings.is_empty() {
            (
                self.report_template.as_deref().unwrap_or(CHECK_PR_TEMPLATE),
                true,
            )
        } else if self.scanner_enabled && self.always_report {
            (
                self.empty_report_template
                    .as_deref()
                    .unwrap_or(CHECK_PR_EMPTY_TEMPLATE),
                true,
            )
        } else {
            return Ok(None);
        };

        let mut context = Context::new();
        context.insert("findings", &self.findings);
        let rendered = Tera::one_off(template, &context, false)
            .map_err(|err| crate::action::check::Error::Template(err.to_string()))?;
        Ok(Some(if trim {
            rendered.trim_end().to_string()
        } else {
            rendered
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn formats_metadata() {
        let metadata = PullRequestMetadata {
            number: 7,
            title: "Fix it".to_string(),
            state: "open".to_string(),
            author: "bob".to_string(),
            draft: false,
            url: "https://github.com/bofa/bofa/pull/7".to_string(),
        };
        assert_eq!(
            format_pr_metadata(&metadata),
            "#7 Fix it by bob [open] https://github.com/bofa/bofa/pull/7"
        );
    }

    #[test]
    fn renders_draft_metadata() {
        let metadata = PullRequestMetadata {
            number: 7,
            title: "Fix it".to_string(),
            state: "open".to_string(),
            author: "bob".to_string(),
            draft: true,
            url: "https://github.com/bofa/bofa/pull/7".to_string(),
        };
        assert_eq!(
            format_pr_metadata(&metadata),
            "#7 Fix it by bob [open] [draft] https://github.com/bofa/bofa/pull/7"
        );
    }

    #[test]
    fn renders_check_pr_output() {
        let metadata = PullRequestMetadata {
            number: 42,
            title: "Fix bug".to_string(),
            state: "closed".to_string(),
            author: "dave".to_string(),
            draft: false,
            url: "https://github.com/owner/repo/pull/42".to_string(),
        };
        let findings = vec![
            SensitiveFinding {
                name: "core-repo".to_string(),
                description: "Core repo".to_string(),
                matched_paths: vec!["/path/to/repo1/src/main.rs".to_string()],
                related_persons: vec!["alice".to_string(), "bob".to_string()],
            },
            SensitiveFinding {
                name: "other".to_string(),
                description: "Other".to_string(),
                matched_paths: vec!["/other/README.md".to_string()],
                related_persons: vec!["carol".to_string()],
            },
        ];
        let result = PrCheckResult {
            metadata,
            findings,
            scanner_enabled: true,
            always_report: false,
            report_template: None,
            empty_report_template: None,
        };
        let expected = r#"Scanner found 2 sensitive groups being changed:
- core-repo: Core repo
  Affected files:
  - `/path/to/repo1/src/main.rs`

  cc @alice @bob
- other: Other
  Affected files:
  - `/other/README.md`

  cc @carol"#;
        assert_eq!(result.render().unwrap(), Some(expected.to_string()));
    }

    #[test]
    fn renders_custom_report_template() {
        let metadata = PullRequestMetadata {
            number: 42,
            title: "Fix bug".to_string(),
            state: "closed".to_string(),
            author: "dave".to_string(),
            draft: false,
            url: "https://github.com/owner/repo/pull/42".to_string(),
        };
        let findings = vec![SensitiveFinding {
            name: "core-repo".to_string(),
            description: "Core repo".to_string(),
            matched_paths: vec!["/path/to/repo1/src/main.rs".to_string()],
            related_persons: vec!["alice".to_string()],
        }];
        let result = PrCheckResult {
            metadata,
            findings,
            scanner_enabled: true,
            always_report: false,
            report_template: Some("{{ findings | length }}: {{ findings[0].name }}".to_string()),
            empty_report_template: None,
        };
        assert_eq!(result.render().unwrap(), Some("1: core-repo".to_string()));
    }

    #[test]
    fn renders_no_findings_as_none() {
        let result = PrCheckResult {
            metadata: PullRequestMetadata {
                number: 1,
                title: "Nothing".to_string(),
                state: "open".to_string(),
                author: "x".to_string(),
                draft: false,
                url: "https://github.com/owner/repo/pull/1".to_string(),
            },
            findings: Vec::new(),
            scanner_enabled: true,
            always_report: false,
            report_template: None,
            empty_report_template: None,
        };
        assert!(result.render().unwrap().is_none());
    }

    #[test]
    fn renders_default_empty_report_when_always_report_enabled() {
        let result = PrCheckResult {
            metadata: PullRequestMetadata {
                number: 1,
                title: "Nothing".to_string(),
                state: "open".to_string(),
                author: "x".to_string(),
                draft: false,
                url: "https://github.com/owner/repo/pull/1".to_string(),
            },
            findings: Vec::new(),
            scanner_enabled: true,
            always_report: true,
            report_template: None,
            empty_report_template: None,
        };
        assert_eq!(
            result.render().unwrap(),
            Some("No sensitive files found.".to_string())
        );
    }

    #[test]
    fn renders_custom_empty_report_when_always_report_enabled() {
        let result = PrCheckResult {
            metadata: PullRequestMetadata {
                number: 1,
                title: "Nothing".to_string(),
                state: "open".to_string(),
                author: "x".to_string(),
                draft: false,
                url: "https://github.com/owner/repo/pull/1".to_string(),
            },
            findings: Vec::new(),
            scanner_enabled: true,
            always_report: true,
            report_template: None,
            empty_report_template: Some("All clear.".to_string()),
        };
        assert_eq!(result.render().unwrap(), Some("All clear.".to_string()));
    }

    #[test]
    fn ignores_always_report_when_scanner_disabled() {
        let result = PrCheckResult {
            metadata: PullRequestMetadata {
                number: 1,
                title: "Nothing".to_string(),
                state: "open".to_string(),
                author: "x".to_string(),
                draft: false,
                url: "https://github.com/owner/repo/pull/1".to_string(),
            },
            findings: Vec::new(),
            scanner_enabled: false,
            always_report: true,
            report_template: None,
            empty_report_template: None,
        };
        assert!(result.render().unwrap().is_none());
    }
}
