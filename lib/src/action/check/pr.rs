use crate::config::repository::RepositoryConfig;
use crate::git::PullRequestMetadata;
use crate::scanner::sensitive::SensitiveFinding;
use std::fmt;

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

#[derive(Debug, Clone)]
pub struct PrCheckResult {
    pub metadata: PullRequestMetadata,
    pub findings: Vec<SensitiveFinding>,
    pub scanner_enabled: bool,
}

impl fmt::Display for PrCheckResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", format_pr_metadata(&self.metadata))?;
        if self.findings.is_empty() {
            if self.scanner_enabled {
                write!(f, "\nNo sensitive files changed.")?;
            }
        } else {
            writeln!(f, "\nSensitive files changed:")?;
            for finding in &self.findings {
                writeln!(f, "  [{}] {}", finding.name, finding.description)?;
                writeln!(f, "    Matched paths: {}", finding.matched_paths.join(", "))?;
                writeln!(
                    f,
                    "    Related persons: {}",
                    finding.related_persons.join(", ")
                )?;
            }
        }
        Ok(())
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
    fn formats_draft_metadata() {
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
}
