use crate::config::repository::RepositoryConfig;
use crate::git::PullRequestMetadata;
use crate::scanner::sensitive::SensitiveFinding;
use crate::templates::{CHECK_PR_EMPTY_TEMPLATE, CHECK_PR_TEMPLATE, COMMENT_FOOTNOTE_TEMPLATE};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentStatus {
    Created,
    Updated,
    Unchanged,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckPrOutput {
    pub body: Option<String>,
    pub status: CommentStatus,
    pub comment_url: Option<String>,
}

pub const COMMENT_MARKER: &str = "<!-- bofa:check-pr -->";

pub fn attach_marker(rendered: &str) -> String {
    format!("{rendered}\n\n{COMMENT_MARKER}")
}

pub fn has_marker(body: &str) -> bool {
    body.contains(COMMENT_MARKER)
}

pub fn strip_marker(body: &str) -> &str {
    match body.find(COMMENT_MARKER) {
        Some(idx) => body[..idx].trim_end(),
        None => body.trim_end(),
    }
}

pub fn content_unchanged(existing_body: &str, new_rendered: &str) -> bool {
    strip_marker(existing_body) == new_rendered.trim_end()
}

#[derive(Debug, Clone)]
pub struct PrCheckResult {
    pub metadata: PullRequestMetadata,
    pub findings: Vec<SensitiveFinding>,
    pub scanner_enabled: bool,
    pub always_report: bool,
    pub report_template: Option<String>,
    pub empty_report_template: Option<String>,
    pub footnote_template: Option<String>,
    pub app_name: Option<String>,
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
        let body = if trim {
            rendered.trim_end().to_string()
        } else {
            rendered
        };
        Ok(Some(self.append_footnote(body)?))
    }

    fn append_footnote(&self, body: String) -> Result<String, crate::action::check::Error> {
        let template = match self.footnote_template.as_deref() {
            Some("") => return Ok(body),
            Some(template) => template,
            None => COMMENT_FOOTNOTE_TEMPLATE,
        };
        let mut context = Context::new();
        context.insert("app_name", &self.app_name);
        let rendered = Tera::one_off(template, &context, false)
            .map_err(|err| crate::action::check::Error::Template(err.to_string()))?;
        Ok(format!("{body}\n\n{}", rendered.trim_end()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attach_marker_appends_hidden_marker() {
        let body = attach_marker("report body");
        assert!(body.starts_with("report body"));
        assert!(has_marker(&body));
        assert!(body.ends_with(COMMENT_MARKER));
    }

    #[test]
    fn has_marker_detects_marker_substring() {
        assert!(has_marker("text <!-- bofa:check-pr -->"));
        assert!(!has_marker("plain text"));
    }

    #[test]
    fn strip_marker_recovers_original_body() {
        let body = attach_marker("report body");
        assert_eq!(strip_marker(&body), "report body");
    }

    #[test]
    fn strip_marker_trims_when_marker_absent() {
        assert_eq!(strip_marker("report body  \n\n"), "report body");
    }

    #[test]
    fn content_unchanged_compares_marker_stripped_content() {
        let existing = attach_marker("report body");
        assert!(content_unchanged(&existing, "report body"));
        assert!(content_unchanged(&existing, "report body\n\n"));
        assert!(!content_unchanged(&existing, "different body"));
    }

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
            footnote_template: Some(String::new()),
            app_name: None,
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
            footnote_template: Some(String::new()),
            app_name: None,
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
            footnote_template: None,
            app_name: None,
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
            footnote_template: Some(String::new()),
            app_name: None,
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
            footnote_template: Some(String::new()),
            app_name: None,
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
            footnote_template: None,
            app_name: None,
        };
        assert!(result.render().unwrap().is_none());
    }

    fn footnote_test_result(
        footnote_template: Option<String>,
        app_name: Option<String>,
    ) -> PrCheckResult {
        PrCheckResult {
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
            footnote_template,
            app_name,
        }
    }

    #[test]
    fn appends_default_footnote_with_app_name() {
        let result = footnote_test_result(None, Some("bofa-app".to_string()));
        assert_eq!(
            result.render().unwrap(),
            Some(
                "No sensitive files found.\n\n<sub>\nThis comment is generated by [bofa](https://github.com/Embers-of-the-Fire/bofa), commented by bofa-app.\n</sub>"
                    .to_string()
            )
        );
    }

    #[test]
    fn omits_app_name_from_footnote_when_missing() {
        let result = footnote_test_result(None, None);
        assert_eq!(
            result.render().unwrap(),
            Some(
                "No sensitive files found.\n\n<sub>\nThis comment is generated by [bofa](https://github.com/Embers-of-the-Fire/bofa).\n</sub>"
                    .to_string()
            )
        );
    }

    #[test]
    fn renders_custom_footnote_template() {
        let result = footnote_test_result(
            Some("powered by {{ app_name }}".to_string()),
            Some("bofa-app".to_string()),
        );
        assert_eq!(
            result.render().unwrap(),
            Some("No sensitive files found.\n\npowered by bofa-app".to_string())
        );
    }
}
