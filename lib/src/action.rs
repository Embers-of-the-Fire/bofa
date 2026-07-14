use crate::config::{BofaConfig, load_config};
use crate::git::backend::{GitBackend, create_backend, dry_run::DryRunBackend};
use crate::git::context::GitContext;
use crate::git::{AccountType, Error as GitError};
use crate::scanner::sensitive::SensitiveScanner;
use crate::scanner::triage::TriageScanner;
use check::pr::{CHECK_COMMENT_MARKER, CommentStatus as CheckCommentStatus};
use std::path::Path;
use thiserror::Error;
use tracing::{debug, info, warn};
use triage::pr::TRIAGE_COMMENT_MARKER;

pub mod check;
pub mod comment_marker;
pub mod triage;

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to load config: {0}")]
    Config(String),
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Check(#[from] check::Error),
    #[error(transparent)]
    Triage(#[from] triage::Error),
}

pub struct Bofa {
    config: BofaConfig,
}

impl Bofa {
    pub fn new(config: BofaConfig) -> Self {
        Self { config }
    }

    pub fn load_config(path: impl AsRef<Path>) -> Result<Self, Error> {
        let config = load_config(path.as_ref()).map_err(|err| Error::Config(err.to_string()))?;
        info!(config_path = %path.as_ref().display(), "loaded config");
        Ok(Self::new(config))
    }

    pub fn config(&self) -> &BofaConfig {
        &self.config
    }

    pub async fn authenticate_with(
        self,
        backend: Box<dyn GitBackend>,
    ) -> Result<AuthenticatedBofa, Error> {
        debug!("authenticating with provided backend");
        let backend = self.wrap_backend(backend);
        let context = GitContext::from_backend(backend);
        Ok(AuthenticatedBofa {
            config: self.config,
            context,
        })
    }

    pub async fn ensure_authenticated(self) -> Result<AuthenticatedBofa, Error> {
        info!(
            provider = ?self.config.worker.provider,
            dry_run = self.config.worker.dry_run,
            "ensuring git authentication"
        );
        let backend = create_backend(
            &self.config.credentials,
            self.config.worker.provider.clone(),
            self.config.worker.dry_run,
        )
        .await?;
        let context = GitContext::from_backend(backend);
        Ok(AuthenticatedBofa {
            config: self.config,
            context,
        })
    }

    fn wrap_backend(&self, backend: Box<dyn GitBackend>) -> Box<dyn GitBackend> {
        if self.config.worker.dry_run {
            Box::new(DryRunBackend::new(backend))
        } else {
            backend
        }
    }

    pub fn with_dry_run(mut self, dry_run: bool) -> Self {
        self.config.worker.dry_run = self.config.worker.dry_run || dry_run;
        self
    }
}

pub struct AuthenticatedBofa {
    config: BofaConfig,
    context: GitContext,
}

trait Labelled {
    fn labels(&self) -> &[String];
}

impl Labelled for crate::scanner::triage::TriageFinding {
    fn labels(&self) -> &[String] {
        &self.labels
    }
}

impl Labelled for crate::scanner::sensitive::SensitiveFinding {
    fn labels(&self) -> &[String] {
        &self.labels
    }
}

trait PrIdentity {
    fn owner(&self) -> &str;
    fn repo(&self) -> &str;
    fn id(&self) -> u64;
}

impl PrIdentity for triage::pr::PrInput {
    fn owner(&self) -> &str {
        &self.owner
    }

    fn repo(&self) -> &str {
        &self.repo
    }

    fn id(&self) -> u64 {
        self.id
    }
}

impl PrIdentity for check::pr::PrInput {
    fn owner(&self) -> &str {
        &self.owner
    }

    fn repo(&self) -> &str {
        &self.repo
    }

    fn id(&self) -> u64 {
        self.id
    }
}

impl AuthenticatedBofa {
    pub fn config(&self) -> &BofaConfig {
        &self.config
    }

    pub fn context(&self) -> &GitContext {
        &self.context
    }

    pub async fn login(&self) -> Result<String, Error> {
        info!("fetching account metadata");
        let metadata = self.context.account_metadata().await?;
        let message = match metadata.account_type {
            AccountType::GitHubApp => {
                let installation = metadata
                    .installation
                    .as_ref()
                    .expect("installation metadata missing for GitHub App");
                format!(
                    "Logged in as {} (GitHub App) installed on {} ({})",
                    metadata.login, installation.login, installation.account_type
                )
            }
            _ => {
                format!(
                    "Logged in as {} ({}), id: {}",
                    metadata.login, metadata.account_type, metadata.id
                )
            }
        };
        Ok(message)
    }

    pub async fn check_pr(&self, id: u64) -> Result<check::pr::CheckPrOutput, Error> {
        let input = check::pr::PrInput::from_repository(id, &self.config.repository);
        info!(
            pr_id = id,
            owner = %input.owner,
            repo = %input.repo,
            "checking pull request"
        );
        let metadata = self
            .context
            .pull_request(&input.owner, &input.repo, input.id)
            .await?;
        let scanner_active = self.config.scanner.enabled && self.config.scanner.sensitive.enabled;
        debug!(scanner_active, "scanner active");
        let mut scanner_active = scanner_active;
        if scanner_active {
            let ignore_patterns = self
                .config
                .scanner
                .ignore
                .iter()
                .map(|pattern| crate::scanner::compile_glob(pattern))
                .collect::<Result<Vec<_>, _>>()
                .map_err(check::Error::InvalidIgnoreGlob)?;
            if crate::scanner::title_ignored(&metadata.title, &ignore_patterns) {
                info!(title = %metadata.title, "pull request title matches ignore pattern, skipping scanner");
                scanner_active = false;
            }
        }
        let findings = if scanner_active {
            let changed_files = self
                .context
                .changed_files(&input.owner, &input.repo, input.id)
                .await?;
            info!(count = changed_files.len(), "fetched changed files");
            let scanner = SensitiveScanner::new(&self.config.scanner.sensitive)
                .map_err(check::Error::from)?;
            let findings = scanner.scan(&changed_files);
            info!(count = findings.len(), "scanner completed");
            findings
        } else {
            debug!("scanner disabled, skipping changed files");
            Vec::new()
        };
        let footnote_template = self.config.template.comment.footnote.clone();
        let will_report =
            !findings.is_empty() || (scanner_active && self.config.scanner.sensitive.always_report);
        let footnote_needs_name = will_report && footnote_template.as_deref() != Some("");
        let posting_needs_name = will_report && self.config.worker.post_comments;
        let account_login = if footnote_needs_name || posting_needs_name {
            match self.context.account_metadata().await {
                Ok(metadata) => Some(metadata.login),
                Err(err) => {
                    warn!(error = %err, "failed to resolve account metadata");
                    None
                }
            }
        } else {
            None
        };
        let app_name = if footnote_needs_name {
            account_login.clone()
        } else {
            None
        };
        let result = check::pr::PrCheckResult {
            metadata,
            findings,
            scanner_enabled: scanner_active,
            always_report: self.config.scanner.sensitive.always_report,
            report_template: self.config.template.scanner.sensitive.report.clone(),
            empty_report_template: self.config.template.scanner.sensitive.empty_report.clone(),
            footnote_template,
            app_name,
        };
        let rendered = result.render()?;
        let (status, comment_url) = if self.config.worker.post_comments {
            if let Some(body) = &rendered {
                let comments = self
                    .context
                    .list_comments(&input.owner, &input.repo, input.id)
                    .await?;
                let existing = comments
                    .into_iter()
                    .filter(|comment| {
                        CHECK_COMMENT_MARKER.has(&comment.body)
                            && account_login
                                .as_ref()
                                .is_some_and(|me| &comment.author_login == me)
                    })
                    .max_by_key(|comment| comment.id);
                match existing {
                    None => {
                        info!("posting new comment to pull request");
                        let url = self
                            .context
                            .post_comment(
                                &input.owner,
                                &input.repo,
                                input.id,
                                &CHECK_COMMENT_MARKER.attach(body),
                            )
                            .await?;
                        (CheckCommentStatus::Created, Some(url))
                    }
                    Some(comment)
                        if CHECK_COMMENT_MARKER.content_unchanged(&comment.body, body) =>
                    {
                        info!("comment unchanged, skipping update");
                        (CheckCommentStatus::Unchanged, Some(comment.url))
                    }
                    Some(comment) => {
                        info!("updating existing comment on pull request");
                        let url = self
                            .context
                            .update_comment(
                                &input.owner,
                                &input.repo,
                                comment.id,
                                &CHECK_COMMENT_MARKER.attach(body),
                            )
                            .await?;
                        (CheckCommentStatus::Updated, Some(url))
                    }
                }
            } else {
                (CheckCommentStatus::Skipped, None)
            }
        } else {
            (CheckCommentStatus::Skipped, None)
        };
        match status {
            CheckCommentStatus::Created => {
                info!(url = %comment_url.as_ref().unwrap(), "comment created")
            }
            CheckCommentStatus::Updated => {
                info!(url = %comment_url.as_ref().unwrap(), "comment updated")
            }
            CheckCommentStatus::Unchanged => {
                info!(url = %comment_url.as_ref().unwrap(), "comment unchanged")
            }
            CheckCommentStatus::Skipped if rendered.is_some() => {
                info!("rendered comment, not posted")
            }
            CheckCommentStatus::Skipped => info!("no sensitive changes detected"),
        }
        let (labels_applied, labels_missing) = self.apply_labels(&input, &result.findings).await?;
        Ok(check::pr::CheckPrOutput {
            body: rendered,
            status,
            comment_url,
            labels_applied,
            labels_missing,
        })
    }

    pub async fn triage_pr(&self, id: u64) -> Result<triage::pr::TriagePrOutput, Error> {
        let input = triage::pr::PrInput::from_repository(id, &self.config.repository);
        info!(
            pr_id = id,
            owner = %input.owner,
            repo = %input.repo,
            "triaging pull request"
        );
        let triage_enabled = self.config.scanner.enabled && self.config.scanner.triage.enabled;
        debug!(triage_enabled, "triage active");
        if !triage_enabled {
            debug!("triage disabled, skipping");
            return Ok(triage::pr::TriagePrOutput {
                body: None,
                status: triage::pr::CommentStatus::Skipped,
                comment_url: None,
                labels_applied: Vec::new(),
                labels_missing: Vec::new(),
            });
        }
        let metadata = self
            .context
            .pull_request(&input.owner, &input.repo, input.id)
            .await?;
        debug!(pr = metadata.number, title = %metadata.title, "fetched pull request metadata");
        let ignore_patterns = self
            .config
            .scanner
            .ignore
            .iter()
            .map(|pattern| crate::scanner::compile_glob(pattern))
            .collect::<Result<Vec<_>, _>>()
            .map_err(triage::Error::InvalidIgnoreGlob)?;
        if crate::scanner::title_ignored(&metadata.title, &ignore_patterns) {
            info!(title = %metadata.title, "pull request title matches ignore pattern, skipping triage");
            return Ok(triage::pr::TriagePrOutput {
                body: None,
                status: triage::pr::CommentStatus::Skipped,
                comment_url: None,
                labels_applied: Vec::new(),
                labels_missing: Vec::new(),
            });
        }
        let changed_files = self
            .context
            .changed_files(&input.owner, &input.repo, input.id)
            .await?;
        info!(count = changed_files.len(), "fetched changed files");
        let scanner =
            TriageScanner::new(&self.config.scanner.triage).map_err(triage::Error::from)?;
        let findings = scanner.scan(&changed_files);
        info!(count = findings.len(), "triage scan completed");
        let footnote_template = self.config.template.comment.footnote.clone();
        let will_report = !findings.is_empty();
        let footnote_needs_name = will_report && footnote_template.as_deref() != Some("");
        let posting_needs_name = will_report && self.config.scanner.triage.post_comment;
        let account_login = if footnote_needs_name || posting_needs_name {
            match self.context.account_metadata().await {
                Ok(metadata) => Some(metadata.login),
                Err(err) => {
                    warn!(error = %err, "failed to resolve account metadata");
                    None
                }
            }
        } else {
            None
        };
        let app_name = if footnote_needs_name {
            account_login.clone()
        } else {
            None
        };
        let result = triage::pr::PrTriageResult {
            findings,
            footnote_template,
            app_name,
        };
        let rendered = result.render()?;
        let (status, comment_url) = if self.config.scanner.triage.post_comment {
            if let Some(body) = &rendered {
                let comments = self
                    .context
                    .list_comments(&input.owner, &input.repo, input.id)
                    .await?;
                let existing = comments
                    .into_iter()
                    .filter(|comment| {
                        TRIAGE_COMMENT_MARKER.has(&comment.body)
                            && account_login
                                .as_ref()
                                .is_some_and(|me| &comment.author_login == me)
                    })
                    .max_by_key(|comment| comment.id);
                match existing {
                    None => {
                        info!("posting new triage comment to pull request");
                        let url = self
                            .context
                            .post_comment(
                                &input.owner,
                                &input.repo,
                                input.id,
                                &TRIAGE_COMMENT_MARKER.attach(body),
                            )
                            .await?;
                        (triage::pr::CommentStatus::Created, Some(url))
                    }
                    Some(comment)
                        if TRIAGE_COMMENT_MARKER.content_unchanged(&comment.body, body) =>
                    {
                        info!("triage comment unchanged, skipping update");
                        (triage::pr::CommentStatus::Unchanged, Some(comment.url))
                    }
                    Some(comment) => {
                        info!("updating existing triage comment on pull request");
                        let url = self
                            .context
                            .update_comment(
                                &input.owner,
                                &input.repo,
                                comment.id,
                                &TRIAGE_COMMENT_MARKER.attach(body),
                            )
                            .await?;
                        (triage::pr::CommentStatus::Updated, Some(url))
                    }
                }
            } else {
                (triage::pr::CommentStatus::Skipped, None)
            }
        } else {
            (triage::pr::CommentStatus::Skipped, None)
        };
        match status {
            triage::pr::CommentStatus::Created => {
                info!(url = %comment_url.as_ref().unwrap(), "triage comment created")
            }
            triage::pr::CommentStatus::Updated => {
                info!(url = %comment_url.as_ref().unwrap(), "triage comment updated")
            }
            triage::pr::CommentStatus::Unchanged => {
                info!(url = %comment_url.as_ref().unwrap(), "triage comment unchanged")
            }
            triage::pr::CommentStatus::Skipped if rendered.is_some() => {
                info!("rendered triage comment, not posted")
            }
            triage::pr::CommentStatus::Skipped => info!("no triage groups matched"),
        }
        let (labels_applied, labels_missing) =
            self.apply_triage_labels(&input, &result.findings).await?;
        Ok(triage::pr::TriagePrOutput {
            body: rendered,
            status,
            comment_url,
            labels_applied,
            labels_missing,
        })
    }

    async fn apply_triage_labels(
        &self,
        input: &triage::pr::PrInput,
        findings: &[crate::scanner::triage::TriageFinding],
    ) -> Result<(Vec<String>, Vec<String>), Error> {
        self.resolve_and_apply_labels(input, Vec::new(), findings, "triage", || true, "")
            .await
    }

    async fn apply_labels(
        &self,
        input: &check::pr::PrInput,
        findings: &[crate::scanner::sensitive::SensitiveFinding],
    ) -> Result<(Vec<String>, Vec<String>), Error> {
        let post_comments = self.config.worker.post_comments;
        self.resolve_and_apply_labels(
            input,
            self.config.scanner.sensitive.labels.clone(),
            findings,
            "",
            || post_comments,
            "post_comments disabled, not applying labels",
        )
        .await
    }

    async fn resolve_and_apply_labels<F>(
        &self,
        input: &impl PrIdentity,
        seed_labels: Vec<String>,
        findings: &[F],
        label_kind: &str,
        add_gate: impl Fn() -> bool,
        skip_log: &str,
    ) -> Result<(Vec<String>, Vec<String>), Error>
    where
        F: Labelled,
    {
        if findings.is_empty() {
            return Ok((Vec::new(), Vec::new()));
        }
        let mut desired = seed_labels;
        for finding in findings {
            for label in finding.labels() {
                if !desired.iter().any(|d| d.eq_ignore_ascii_case(label)) {
                    desired.push(label.clone());
                }
            }
        }
        if desired.is_empty() {
            return Ok((Vec::new(), Vec::new()));
        }
        let kind_prefix = if label_kind.is_empty() {
            String::new()
        } else {
            format!("{label_kind} ")
        };
        info!(
            count = desired.len(),
            "resolving {}labels for pull request", kind_prefix
        );
        let existing = self
            .context
            .list_labels(input.owner(), input.repo())
            .await?;
        let mut applicable = Vec::new();
        let mut missing = Vec::new();
        for label in desired {
            match existing.iter().find(|e| e.eq_ignore_ascii_case(&label)) {
                Some(canonical) => applicable.push(canonical.clone()),
                None => missing.push(label),
            }
        }
        if !missing.is_empty() {
            warn!(
                missing = ?missing,
                "configured {}labels not present in repository, skipping them",
                kind_prefix
            );
        }
        if applicable.is_empty() {
            return Ok((Vec::new(), missing));
        }
        if !add_gate() {
            info!(count = applicable.len(), "{skip_log}");
            return Ok((Vec::new(), missing));
        }
        info!(
            count = applicable.len(),
            "adding {}labels to pull request", kind_prefix
        );
        self.context
            .add_labels(input.owner(), input.repo(), input.id(), &applicable)
            .await?;
        Ok((applicable, missing))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::check::pr::{CHECK_COMMENT_MARKER, CommentStatus};
    use crate::config::BofaConfig;
    use crate::config::credentials::{Credentials, PersonalTokenCredentials, SecretString};
    use crate::config::scanner::sensitive::{SensitiveScannerConfig, SensitiveScannerItem};
    use crate::config::scanner::triage::{TriageConfig, TriageGroup};
    use crate::git::backend::mock::MockGitBackend;
    use crate::git::{ChangedFile, FileChangeStatus, PullRequestMetadata};

    fn test_config() -> BofaConfig {
        BofaConfig {
            credentials: Credentials::PersonalToken(PersonalTokenCredentials {
                token: SecretString::new("$DUMMY_TOKEN"),
            }),
            repository: crate::config::repository::RepositoryConfig {
                owner: "owner".to_string(),
                repo: "repo".to_string(),
            },
            worker: Default::default(),
            scanner: Default::default(),
            template: Default::default(),
            log: Default::default(),
        }
    }

    fn config_with_sensitive_scanner() -> BofaConfig {
        let mut config = test_config();
        config.scanner.sensitive = SensitiveScannerConfig {
            enabled: true,
            always_report: false,
            labels: Vec::new(),
            groups: indexmap::indexmap! {
                "core-repo".to_string() => SensitiveScannerItem {
                    description: "Core repo".to_string(),
                    paths: vec!["/path/to/repo1/**".to_string()],
                    members: vec!["alice".to_string(), "bob".to_string()],
                    labels: Vec::new(),
                },
                "other".to_string() => SensitiveScannerItem {
                    description: "Other".to_string(),
                    paths: vec!["/other/**".to_string()],
                    members: vec!["carol".to_string()],
                    labels: Vec::new(),
                },
            },
        };
        config
    }

    fn changed_file(path: &str) -> ChangedFile {
        ChangedFile {
            path: path.to_string(),
            status: FileChangeStatus::Modified,
        }
    }

    fn config_with_always_report() -> BofaConfig {
        let mut config = config_with_sensitive_scanner();
        config.scanner.sensitive.always_report = true;
        config
    }

    const EMPTY_REPORT_RENDERED: &str = "No sensitive files found.\n\n<sub>\n\nThis comment is generated by [bofa](https://github.com/Embers-of-the-Fire/bofa), commented by @octocat.\n\n</sub>";

    fn marked_comment(id: u64, author: &str, rendered: &str) -> crate::git::IssueComment {
        crate::git::IssueComment {
            id,
            body: CHECK_COMMENT_MARKER.attach(rendered),
            author_login: author.to_string(),
            url: format!("https://github.com/owner/repo/pull/42#issuecomment-{id}"),
        }
    }

    #[tokio::test]
    async fn login_propagates_backend_error() {
        let backend = Box::new(MockGitBackend::with_account_metadata(|| async {
            Err(GitError::Api("boom".to_string()))
        }));
        let bofa = Bofa::new(test_config())
            .authenticate_with(backend)
            .await
            .unwrap();
        let err = bofa.login().await.unwrap_err();
        assert!(matches!(err, Error::Git(GitError::Api(_))));
    }

    #[tokio::test]
    async fn check_pr_propagates_backend_error() {
        let backend = MockGitBackend::with_account_metadata(|| async {
            Ok(crate::git::AccountMetadata {
                id: 1,
                login: "alice".to_string(),
                account_type: AccountType::User,
                installation: None,
            })
        });
        backend.set_pull_request(|| async { Err(GitError::Api("boom".to_string())) });
        let backend = Box::new(backend);
        let bofa = Bofa::new(test_config())
            .authenticate_with(backend)
            .await
            .unwrap();
        let err = bofa.check_pr(1).await.unwrap_err();
        assert!(matches!(err, Error::Git(GitError::Api(_))));
    }

    #[tokio::test]
    async fn check_pr_returns_none_when_scanner_disabled() {
        let backend = Box::new(MockGitBackend::new());
        let bofa = Bofa::new(test_config())
            .authenticate_with(backend)
            .await
            .unwrap();
        let output = bofa.check_pr(1).await.unwrap();
        assert!(output.body.is_none());
        assert_eq!(output.status, CommentStatus::Skipped);
        assert!(output.comment_url.is_none());
    }

    #[tokio::test]
    async fn check_pr_reports_sensitive_files_and_related_persons() {
        let backend = MockGitBackend::new();
        backend.set_pull_request(|| async {
            Ok(PullRequestMetadata {
                number: 42,
                title: "Fix bug".to_string(),
                state: "closed".to_string(),
                author: "dave".to_string(),
                draft: false,
                url: "https://github.com/owner/repo/pull/42".to_string(),
            })
        });
        backend.set_changed_files(|| async {
            Ok(vec![
                changed_file("/path/to/repo1/src/main.rs"),
                changed_file("/other/README.md"),
            ])
        });
        let bofa = Bofa::new(config_with_sensitive_scanner())
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.check_pr(42).await.unwrap();
        let body = output.body.unwrap();
        assert!(body.contains("Core repo"));
        assert!(body.contains("/path/to/repo1/src/main.rs"));
        assert!(body.contains("alice"));
        assert!(body.contains("bob"));
        assert!(body.contains("Other"));
        assert!(body.contains("/other/README.md"));
        assert!(body.contains("carol"));
        assert_eq!(output.status, CommentStatus::Created);
        assert!(output.comment_url.is_some());
    }

    #[tokio::test]
    async fn check_pr_returns_none_when_no_sensitive_files_changed() {
        let backend = MockGitBackend::new();
        backend.set_changed_files(|| async { Ok(vec![changed_file("/unrelated/file.txt")]) });
        let bofa = Bofa::new(config_with_sensitive_scanner())
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.check_pr(42).await.unwrap();
        assert!(output.body.is_none());
        assert_eq!(output.status, CommentStatus::Skipped);
        assert!(output.comment_url.is_none());
    }

    #[tokio::test]
    async fn check_pr_propagates_changed_files_error() {
        let backend = MockGitBackend::new();
        backend.set_changed_files(|| async { Err(GitError::Api("diff boom".to_string())) });
        let bofa = Bofa::new(config_with_sensitive_scanner())
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let err = bofa.check_pr(42).await.unwrap_err();
        assert!(matches!(err, Error::Git(GitError::Api(_))));
    }

    #[tokio::test]
    async fn check_pr_skips_scan_when_title_matches_ignore_pattern() {
        let backend = MockGitBackend::new();
        backend.set_pull_request(|| async {
            Ok(PullRequestMetadata {
                number: 42,
                title: "chore(deps): bump serde".to_string(),
                state: "closed".to_string(),
                author: "dave".to_string(),
                draft: false,
                url: "https://github.com/owner/repo/pull/42".to_string(),
            })
        });
        backend.set_changed_files(|| async {
            Ok(vec![
                changed_file("/path/to/repo1/src/main.rs"),
                changed_file("/other/README.md"),
            ])
        });
        let mut config = config_with_sensitive_scanner();
        config.scanner.ignore = vec!["chore(deps):*".to_string()];
        let bofa = Bofa::new(config)
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.check_pr(42).await.unwrap();
        assert!(output.body.is_none());
        assert_eq!(output.status, CommentStatus::Skipped);
        assert!(output.comment_url.is_none());
    }

    #[tokio::test]
    async fn check_pr_ignore_matching_is_case_insensitive() {
        let backend = MockGitBackend::new();
        backend.set_pull_request(|| async {
            Ok(PullRequestMetadata {
                number: 42,
                title: "CHORE(DEPS): bump serde".to_string(),
                state: "closed".to_string(),
                author: "dave".to_string(),
                draft: false,
                url: "https://github.com/owner/repo/pull/42".to_string(),
            })
        });
        backend
            .set_changed_files(|| async { Ok(vec![changed_file("/path/to/repo1/src/main.rs")]) });
        let mut config = config_with_sensitive_scanner();
        config.scanner.ignore = vec!["chore(deps):*".to_string()];
        let bofa = Bofa::new(config)
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.check_pr(42).await.unwrap();
        assert!(output.body.is_none());
        assert_eq!(output.status, CommentStatus::Skipped);
    }

    #[tokio::test]
    async fn check_pr_returns_invalid_ignore_glob_error() {
        let backend = MockGitBackend::new();
        backend.set_pull_request(|| async {
            Ok(PullRequestMetadata {
                number: 42,
                title: "Fix bug".to_string(),
                state: "closed".to_string(),
                author: "dave".to_string(),
                draft: false,
                url: "https://github.com/owner/repo/pull/42".to_string(),
            })
        });
        let mut config = config_with_sensitive_scanner();
        config.scanner.ignore = vec!["src/[[*.rs".to_string()];
        let bofa = Bofa::new(config)
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let err = bofa.check_pr(42).await.unwrap_err();
        assert!(matches!(
            err,
            Error::Check(check::Error::InvalidIgnoreGlob(_))
        ));
    }

    #[tokio::test]
    async fn dry_run_blocks_delete_branch() {
        let mut config = test_config();
        config.worker.dry_run = true;
        let bofa = Bofa::new(config)
            .authenticate_with(Box::new(MockGitBackend::new()))
            .await
            .unwrap();
        let err = bofa
            .context()
            .delete_branch("owner", "repo", "feature")
            .await
            .unwrap_err();
        assert!(matches!(err, GitError::DryRun(_)));
    }

    #[tokio::test]
    async fn dry_run_blocks_publish_release() {
        let mut config = test_config();
        config.worker.dry_run = true;
        let bofa = Bofa::new(config)
            .authenticate_with(Box::new(MockGitBackend::new()))
            .await
            .unwrap();
        let err = bofa
            .context()
            .publish_release("owner", "repo", "v1.0.0")
            .await
            .unwrap_err();
        assert!(matches!(err, GitError::DryRun(_)));
    }

    #[tokio::test]
    async fn dry_run_blocks_upload_file() {
        let mut config = test_config();
        config.worker.dry_run = true;
        let bofa = Bofa::new(config)
            .authenticate_with(Box::new(MockGitBackend::new()))
            .await
            .unwrap();
        let err = bofa
            .context()
            .upload_file("owner", "repo", "path.txt", b"content")
            .await
            .unwrap_err();
        assert!(matches!(err, GitError::DryRun(_)));
    }

    #[tokio::test]
    async fn non_fetch_actions_reach_backend_when_not_dry_run() {
        let bofa = Bofa::new(test_config())
            .authenticate_with(Box::new(MockGitBackend::new()))
            .await
            .unwrap();
        let err = bofa
            .context()
            .delete_branch("owner", "repo", "feature")
            .await
            .unwrap_err();
        assert!(matches!(err, GitError::Unsupported(_)));
    }

    #[tokio::test]
    async fn fetch_actions_work_in_dry_run() {
        let mut config = test_config();
        config.worker.dry_run = true;
        let bofa = Bofa::new(config)
            .authenticate_with(Box::new(MockGitBackend::new()))
            .await
            .unwrap();
        let output = bofa.check_pr(1).await.unwrap();
        assert!(output.body.is_none());
        assert_eq!(output.status, CommentStatus::Skipped);
        assert!(output.comment_url.is_none());
    }

    #[tokio::test]
    async fn check_pr_posts_comment_when_enabled_and_sensitive_files_found() {
        let backend = MockGitBackend::new();
        backend.set_pull_request(|| async {
            Ok(PullRequestMetadata {
                number: 42,
                title: "Fix bug".to_string(),
                state: "closed".to_string(),
                author: "dave".to_string(),
                draft: false,
                url: "https://github.com/owner/repo/pull/42".to_string(),
            })
        });
        backend
            .set_changed_files(|| async { Ok(vec![changed_file("/path/to/repo1/src/main.rs")]) });
        let bofa = Bofa::new(config_with_sensitive_scanner())
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.check_pr(42).await.unwrap();
        assert!(output.body.is_some());
        assert_eq!(output.status, CommentStatus::Created);
        assert_eq!(
            output.comment_url,
            Some("https://github.com/test/repo/pull/1#issuecomment-1".to_string())
        );
    }

    #[tokio::test]
    async fn check_pr_does_not_post_comment_when_disabled() {
        let backend = MockGitBackend::new();
        backend.set_pull_request(|| async {
            Ok(PullRequestMetadata {
                number: 42,
                title: "Fix bug".to_string(),
                state: "closed".to_string(),
                author: "dave".to_string(),
                draft: false,
                url: "https://github.com/owner/repo/pull/42".to_string(),
            })
        });
        backend
            .set_changed_files(|| async { Ok(vec![changed_file("/path/to/repo1/src/main.rs")]) });
        let mut config = config_with_sensitive_scanner();
        config.worker.post_comments = false;
        let bofa = Bofa::new(config)
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.check_pr(42).await.unwrap();
        assert!(output.body.is_some());
        assert_eq!(output.status, CommentStatus::Skipped);
        assert!(output.comment_url.is_none());
    }

    #[tokio::test]
    async fn check_pr_does_not_scan_when_scanner_master_disabled() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let call_count = Arc::new(AtomicU32::new(0));
        let call_count_clone = Arc::clone(&call_count);
        let backend = MockGitBackend::new();
        backend.set_changed_files(move || {
            let call_count = Arc::clone(&call_count_clone);
            async move {
                call_count.fetch_add(1, Ordering::SeqCst);
                Ok(vec![changed_file("/path/to/repo1/src/main.rs")])
            }
        });
        let mut config = config_with_sensitive_scanner();
        config.scanner.enabled = false;
        let bofa = Bofa::new(config)
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.check_pr(42).await.unwrap();
        assert!(output.body.is_none());
        assert_eq!(output.status, CommentStatus::Skipped);
        assert!(output.comment_url.is_none());
        assert_eq!(call_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn check_pr_skips_update_when_comment_unchanged() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let backend = MockGitBackend::new();
        backend.set_changed_files(|| async { Ok(vec![changed_file("/unrelated/file.txt")]) });
        backend.set_list_comments(|| async {
            Ok(vec![marked_comment(7, "octocat", EMPTY_REPORT_RENDERED)])
        });
        let post_calls = Arc::new(AtomicU32::new(0));
        let update_calls = Arc::new(AtomicU32::new(0));
        let pc = Arc::clone(&post_calls);
        backend.set_post_comment(move || {
            let pc = Arc::clone(&pc);
            async move {
                pc.fetch_add(1, Ordering::SeqCst);
                Ok("https://created".to_string())
            }
        });
        let uc = Arc::clone(&update_calls);
        backend.set_update_comment(move || {
            let uc = Arc::clone(&uc);
            async move {
                uc.fetch_add(1, Ordering::SeqCst);
                Ok("https://updated".to_string())
            }
        });
        let bofa = Bofa::new(config_with_always_report())
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.check_pr(42).await.unwrap();
        assert_eq!(output.status, CommentStatus::Unchanged);
        assert_eq!(
            output.comment_url,
            Some("https://github.com/owner/repo/pull/42#issuecomment-7".to_string())
        );
        assert_eq!(post_calls.load(Ordering::SeqCst), 0);
        assert_eq!(update_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn check_pr_updates_existing_marked_comment_when_content_changes() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let backend = MockGitBackend::new();
        backend.set_changed_files(|| async { Ok(vec![changed_file("/unrelated/file.txt")]) });
        backend
            .set_list_comments(|| async { Ok(vec![marked_comment(7, "octocat", "stale report")]) });
        let post_calls = Arc::new(AtomicU32::new(0));
        let pc = Arc::clone(&post_calls);
        backend.set_post_comment(move || {
            let pc = Arc::clone(&pc);
            async move {
                pc.fetch_add(1, Ordering::SeqCst);
                Ok("https://created".to_string())
            }
        });
        backend.set_update_comment(|| async {
            Ok("https://github.com/owner/repo/pull/42#issuecomment-7".to_string())
        });
        let bofa = Bofa::new(config_with_always_report())
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.check_pr(42).await.unwrap();
        assert_eq!(output.status, CommentStatus::Updated);
        assert_eq!(
            output.comment_url,
            Some("https://github.com/owner/repo/pull/42#issuecomment-7".to_string())
        );
        assert_eq!(post_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn check_pr_creates_new_comment_when_existing_comment_lacks_marker() {
        let backend = MockGitBackend::new();
        backend.set_changed_files(|| async { Ok(vec![changed_file("/unrelated/file.txt")]) });
        backend.set_list_comments(|| async {
            Ok(vec![crate::git::IssueComment {
                id: 3,
                body: "a human comment".to_string(),
                author_login: "octocat".to_string(),
                url: "https://github.com/owner/repo/pull/42#issuecomment-3".to_string(),
            }])
        });
        let bofa = Bofa::new(config_with_always_report())
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.check_pr(42).await.unwrap();
        assert_eq!(output.status, CommentStatus::Created);
        assert_eq!(
            output.comment_url,
            Some("https://github.com/test/repo/pull/1#issuecomment-1".to_string())
        );
    }

    #[tokio::test]
    async fn check_pr_creates_new_comment_when_marked_comment_is_from_another_author() {
        let backend = MockGitBackend::new();
        backend.set_changed_files(|| async { Ok(vec![changed_file("/unrelated/file.txt")]) });
        backend.set_list_comments(|| async {
            Ok(vec![marked_comment(
                9,
                "someone-else",
                EMPTY_REPORT_RENDERED,
            )])
        });
        let bofa = Bofa::new(config_with_always_report())
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.check_pr(42).await.unwrap();
        assert_eq!(output.status, CommentStatus::Created);
        assert_eq!(
            output.comment_url,
            Some("https://github.com/test/repo/pull/1#issuecomment-1".to_string())
        );
    }

    #[tokio::test]
    async fn check_pr_creates_new_comment_when_account_metadata_fails() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let backend = MockGitBackend::new();
        backend.set_account_metadata(|| async { Err(GitError::Api("metadata boom".to_string())) });
        backend.set_changed_files(|| async { Ok(vec![changed_file("/unrelated/file.txt")]) });
        backend
            .set_list_comments(|| async { Ok(vec![marked_comment(7, "octocat", "stale report")]) });
        let update_calls = Arc::new(AtomicU32::new(0));
        let uc = Arc::clone(&update_calls);
        backend.set_update_comment(move || {
            let uc = Arc::clone(&uc);
            async move {
                uc.fetch_add(1, Ordering::SeqCst);
                Ok("https://updated".to_string())
            }
        });
        let bofa = Bofa::new(config_with_always_report())
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.check_pr(42).await.unwrap();
        assert_eq!(output.status, CommentStatus::Created);
        assert_eq!(
            output.comment_url,
            Some("https://github.com/test/repo/pull/1#issuecomment-1".to_string())
        );
        assert_eq!(update_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn check_pr_updates_most_recent_marked_comment() {
        let backend = MockGitBackend::new();
        backend.set_changed_files(|| async { Ok(vec![changed_file("/unrelated/file.txt")]) });
        backend.set_list_comments(|| async {
            Ok(vec![
                marked_comment(2, "octocat", "stale report"),
                marked_comment(5, "octocat", EMPTY_REPORT_RENDERED),
            ])
        });
        let bofa = Bofa::new(config_with_always_report())
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.check_pr(42).await.unwrap();
        assert_eq!(output.status, CommentStatus::Unchanged);
        assert_eq!(
            output.comment_url,
            Some("https://github.com/owner/repo/pull/42#issuecomment-5".to_string())
        );
    }

    fn config_with_labels() -> BofaConfig {
        let mut config = test_config();
        config.scanner.sensitive = SensitiveScannerConfig {
            enabled: true,
            always_report: false,
            labels: vec!["needs-security-review".to_string()],
            groups: indexmap::indexmap! {
                "core-repo".to_string() => SensitiveScannerItem {
                    description: "Core repo".to_string(),
                    paths: vec!["/path/to/repo1/**".to_string()],
                    members: vec!["alice".to_string()],
                    labels: vec!["core-impact".to_string(), "needs-security-review".to_string()],
                },
                "other".to_string() => SensitiveScannerItem {
                    description: "Other".to_string(),
                    paths: vec!["/other/**".to_string()],
                    members: vec!["carol".to_string()],
                    labels: vec!["other-impact".to_string()],
                },
            },
        };
        config
    }

    #[tokio::test]
    async fn check_pr_applies_labels_when_findings_exist() {
        let backend = MockGitBackend::new();
        backend.set_changed_files(|| async {
            Ok(vec![
                changed_file("/path/to/repo1/src/main.rs"),
                changed_file("/other/README.md"),
            ])
        });
        backend.set_list_labels(|| async {
            Ok(vec![
                "needs-security-review".to_string(),
                "core-impact".to_string(),
                "other-impact".to_string(),
            ])
        });
        let bofa = Bofa::new(config_with_labels())
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.check_pr(42).await.unwrap();
        assert_eq!(
            output.labels_applied,
            vec![
                "needs-security-review".to_string(),
                "core-impact".to_string(),
                "other-impact".to_string(),
            ]
        );
        assert!(output.labels_missing.is_empty());
    }

    #[tokio::test]
    async fn check_pr_does_not_apply_labels_when_no_findings() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let backend = MockGitBackend::new();
        backend.set_changed_files(|| async { Ok(vec![changed_file("/unrelated/file.txt")]) });
        let list_calls = Arc::new(AtomicU32::new(0));
        let lc = Arc::clone(&list_calls);
        backend.set_list_labels(move || {
            let lc = Arc::clone(&lc);
            async move {
                lc.fetch_add(1, Ordering::SeqCst);
                Ok(Vec::new())
            }
        });
        let add_calls = Arc::new(AtomicU32::new(0));
        let ac = Arc::clone(&add_calls);
        backend.set_add_labels(move || {
            let ac = Arc::clone(&ac);
            async move {
                ac.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        });
        let bofa = Bofa::new(config_with_labels())
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.check_pr(42).await.unwrap();
        assert!(output.labels_applied.is_empty());
        assert!(output.labels_missing.is_empty());
        assert_eq!(list_calls.load(Ordering::SeqCst), 0);
        assert_eq!(add_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn check_pr_matches_labels_case_insensitively() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let backend = MockGitBackend::new();
        backend
            .set_changed_files(|| async { Ok(vec![changed_file("/path/to/repo1/src/main.rs")]) });
        backend.set_list_labels(|| async {
            Ok(vec![
                "Needs-Security-Review".to_string(),
                "CORE-IMPACT".to_string(),
            ])
        });
        let add_calls = Arc::new(AtomicU32::new(0));
        let ac = Arc::clone(&add_calls);
        backend.set_add_labels(move || {
            let ac = Arc::clone(&ac);
            async move {
                ac.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        });
        let bofa = Bofa::new(config_with_labels())
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.check_pr(42).await.unwrap();
        assert_eq!(
            output.labels_applied,
            vec![
                "Needs-Security-Review".to_string(),
                "CORE-IMPACT".to_string(),
            ]
        );
        assert!(output.labels_missing.is_empty());
        assert_eq!(add_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn check_pr_skips_missing_labels_and_applies_existing() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let backend = MockGitBackend::new();
        backend
            .set_changed_files(|| async { Ok(vec![changed_file("/path/to/repo1/src/main.rs")]) });
        backend.set_list_labels(|| async { Ok(vec!["core-impact".to_string()]) });
        let add_calls = Arc::new(AtomicU32::new(0));
        let ac = Arc::clone(&add_calls);
        backend.set_add_labels(move || {
            let ac = Arc::clone(&ac);
            async move {
                ac.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        });
        let bofa = Bofa::new(config_with_labels())
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.check_pr(42).await.unwrap();
        assert_eq!(output.labels_applied, vec!["core-impact".to_string()]);
        assert_eq!(
            output.labels_missing,
            vec!["needs-security-review".to_string()]
        );
        assert_eq!(add_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn check_pr_checks_but_does_not_apply_labels_when_post_comments_disabled() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let backend = MockGitBackend::new();
        backend
            .set_changed_files(|| async { Ok(vec![changed_file("/path/to/repo1/src/main.rs")]) });
        let list_calls = Arc::new(AtomicU32::new(0));
        let lc = Arc::clone(&list_calls);
        backend.set_list_labels(move || {
            let lc = Arc::clone(&lc);
            async move {
                lc.fetch_add(1, Ordering::SeqCst);
                Ok(Vec::new())
            }
        });
        let add_calls = Arc::new(AtomicU32::new(0));
        let ac = Arc::clone(&add_calls);
        backend.set_add_labels(move || {
            let ac = Arc::clone(&ac);
            async move {
                ac.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        });
        let mut config = config_with_labels();
        config.worker.post_comments = false;
        let bofa = Bofa::new(config)
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.check_pr(42).await.unwrap();
        assert!(output.labels_applied.is_empty());
        assert_eq!(
            output.labels_missing,
            vec![
                "needs-security-review".to_string(),
                "core-impact".to_string(),
            ]
        );
        assert_eq!(list_calls.load(Ordering::SeqCst), 1);
        assert_eq!(add_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn check_pr_does_not_apply_existing_labels_when_post_comments_disabled() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let backend = MockGitBackend::new();
        backend
            .set_changed_files(|| async { Ok(vec![changed_file("/path/to/repo1/src/main.rs")]) });
        backend.set_list_labels(|| async {
            Ok(vec![
                "needs-security-review".to_string(),
                "core-impact".to_string(),
            ])
        });
        let add_calls = Arc::new(AtomicU32::new(0));
        let ac = Arc::clone(&add_calls);
        backend.set_add_labels(move || {
            let ac = Arc::clone(&ac);
            async move {
                ac.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        });
        let mut config = config_with_labels();
        config.worker.post_comments = false;
        let bofa = Bofa::new(config)
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.check_pr(42).await.unwrap();
        assert!(output.labels_applied.is_empty());
        assert!(output.labels_missing.is_empty());
        assert_eq!(add_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn check_pr_dry_run_blocks_add_labels() {
        let mut config = config_with_labels();
        let scanner = SensitiveScanner::new(&config.scanner.sensitive).unwrap();
        let findings = scanner.scan(&[changed_file("/path/to/repo1/src/main.rs")]);
        let rendered = check::pr::PrCheckResult {
            metadata: PullRequestMetadata {
                number: 1,
                title: String::new(),
                state: String::new(),
                author: String::new(),
                draft: false,
                url: String::new(),
            },
            findings,
            scanner_enabled: true,
            always_report: false,
            report_template: None,
            empty_report_template: None,
            footnote_template: None,
            app_name: Some("octocat".to_string()),
        }
        .render()
        .unwrap()
        .unwrap();

        let backend = MockGitBackend::new();
        backend
            .set_changed_files(|| async { Ok(vec![changed_file("/path/to/repo1/src/main.rs")]) });
        backend.set_list_comments(move || {
            let rendered = rendered.clone();
            async move { Ok(vec![marked_comment(7, "octocat", &rendered)]) }
        });
        backend.set_list_labels(|| async { Ok(vec!["core-impact".to_string()]) });
        config.worker.dry_run = true;
        let bofa = Bofa::new(config)
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let err = bofa.check_pr(42).await.unwrap_err();
        assert!(matches!(err, Error::Git(GitError::DryRun(action)) if action == "add_labels"));
    }

    #[tokio::test]
    async fn check_pr_propagates_list_labels_error() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let backend = MockGitBackend::new();
        backend
            .set_changed_files(|| async { Ok(vec![changed_file("/path/to/repo1/src/main.rs")]) });
        backend.set_list_labels(|| async { Err(GitError::Api("labels boom".to_string())) });
        let add_calls = Arc::new(AtomicU32::new(0));
        let ac = Arc::clone(&add_calls);
        backend.set_add_labels(move || {
            let ac = Arc::clone(&ac);
            async move {
                ac.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        });
        let bofa = Bofa::new(config_with_labels())
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let err = bofa.check_pr(42).await.unwrap_err();
        assert!(matches!(err, Error::Git(GitError::Api(_))));
        assert_eq!(add_calls.load(Ordering::SeqCst), 0);
    }

    fn config_with_triage() -> BofaConfig {
        let mut config = test_config();
        config.scanner.triage = TriageConfig {
            enabled: true,
            post_comment: false,
            groups: indexmap::indexmap! {
                "core".to_string() => TriageGroup {
                    description: "Core changes".to_string(),
                    paths: vec!["/path/to/repo1/**".to_string()],
                    labels: vec!["core-impact".to_string()],
                },
                "docs".to_string() => TriageGroup {
                    description: "Documentation".to_string(),
                    paths: vec!["/docs/**".to_string()],
                    labels: vec!["docs".to_string()],
                },
            },
        };
        config
    }

    fn triage_marked_comment(id: u64, author: &str, rendered: &str) -> crate::git::IssueComment {
        crate::git::IssueComment {
            id,
            body: TRIAGE_COMMENT_MARKER.attach(rendered),
            author_login: author.to_string(),
            url: format!("https://github.com/owner/repo/pull/42#issuecomment-{id}"),
        }
    }

    #[tokio::test]
    async fn triage_pr_disabled_returns_skipped() {
        let backend = MockGitBackend::new();
        let bofa = Bofa::new(test_config())
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.triage_pr(42).await.unwrap();
        assert!(output.body.is_none());
        assert_eq!(output.status, triage::pr::CommentStatus::Skipped);
        assert!(output.comment_url.is_none());
        assert!(output.labels_applied.is_empty());
        assert!(output.labels_missing.is_empty());
    }

    #[tokio::test]
    async fn triage_pr_no_match_returns_skipped() {
        let backend = MockGitBackend::new();
        backend.set_changed_files(|| async { Ok(vec![changed_file("/unrelated/file.txt")]) });
        let bofa = Bofa::new(config_with_triage())
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.triage_pr(42).await.unwrap();
        assert!(output.body.is_none());
        assert_eq!(output.status, triage::pr::CommentStatus::Skipped);
        assert!(output.comment_url.is_none());
        assert!(output.labels_applied.is_empty());
        assert!(output.labels_missing.is_empty());
    }

    #[tokio::test]
    async fn triage_pr_applies_labels_when_group_matches() {
        let backend = MockGitBackend::new();
        backend
            .set_changed_files(|| async { Ok(vec![changed_file("/path/to/repo1/src/main.rs")]) });
        backend.set_list_labels(|| async { Ok(vec!["core-impact".to_string()]) });
        let bofa = Bofa::new(config_with_triage())
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.triage_pr(42).await.unwrap();
        assert_eq!(output.labels_applied, vec!["core-impact".to_string()]);
        assert!(output.labels_missing.is_empty());
    }

    #[tokio::test]
    async fn triage_pr_skips_when_title_matches_ignore_pattern() {
        let backend = MockGitBackend::new();
        backend.set_pull_request(|| async {
            Ok(PullRequestMetadata {
                number: 42,
                title: "chore(deps): bump serde".to_string(),
                state: "closed".to_string(),
                author: "dave".to_string(),
                draft: false,
                url: "https://github.com/owner/repo/pull/42".to_string(),
            })
        });
        backend
            .set_changed_files(|| async { Ok(vec![changed_file("/path/to/repo1/src/main.rs")]) });
        backend.set_list_labels(|| async { Ok(vec!["core-impact".to_string()]) });
        let mut config = config_with_triage();
        config.scanner.ignore = vec!["chore(deps):*".to_string()];
        let bofa = Bofa::new(config)
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.triage_pr(42).await.unwrap();
        assert!(output.body.is_none());
        assert_eq!(output.status, triage::pr::CommentStatus::Skipped);
        assert!(output.comment_url.is_none());
        assert!(output.labels_applied.is_empty());
        assert!(output.labels_missing.is_empty());
    }

    #[tokio::test]
    async fn triage_pr_returns_invalid_ignore_glob_error() {
        let backend = MockGitBackend::new();
        backend.set_pull_request(|| async {
            Ok(PullRequestMetadata {
                number: 42,
                title: "Fix bug".to_string(),
                state: "closed".to_string(),
                author: "dave".to_string(),
                draft: false,
                url: "https://github.com/owner/repo/pull/42".to_string(),
            })
        });
        let mut config = config_with_triage();
        config.scanner.ignore = vec!["src/[[*.rs".to_string()];
        let bofa = Bofa::new(config)
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let err = bofa.triage_pr(42).await.unwrap_err();
        assert!(matches!(
            err,
            Error::Triage(triage::Error::InvalidIgnoreGlob(_))
        ));
    }

    #[tokio::test]
    async fn triage_pr_posts_comment_when_post_comment_enabled() {
        let mut config = config_with_triage();
        config.scanner.triage.post_comment = true;
        let backend = MockGitBackend::new();
        backend
            .set_changed_files(|| async { Ok(vec![changed_file("/path/to/repo1/src/main.rs")]) });
        backend.set_list_labels(|| async { Ok(vec!["core-impact".to_string()]) });
        let bofa = Bofa::new(config)
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.triage_pr(42).await.unwrap();
        assert_eq!(output.status, triage::pr::CommentStatus::Created);
        assert!(output.body.is_some());
        assert!(
            output
                .body
                .as_ref()
                .unwrap()
                .contains("Automatically triage the pull request to:")
        );
        assert!(output.labels_applied.contains(&"core-impact".to_string()));
        assert_eq!(
            output.comment_url,
            Some("https://github.com/test/repo/pull/1#issuecomment-1".to_string())
        );
    }

    #[tokio::test]
    async fn triage_pr_applies_labels_without_posting_comment() {
        let backend = MockGitBackend::new();
        backend
            .set_changed_files(|| async { Ok(vec![changed_file("/path/to/repo1/src/main.rs")]) });
        backend.set_list_labels(|| async { Ok(vec!["core-impact".to_string()]) });
        let bofa = Bofa::new(config_with_triage())
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.triage_pr(42).await.unwrap();
        assert_eq!(output.status, triage::pr::CommentStatus::Skipped);
        assert!(output.body.is_some());
        assert!(output.labels_applied.contains(&"core-impact".to_string()));
        assert!(output.comment_url.is_none());
    }

    #[tokio::test]
    async fn triage_pr_matches_labels_case_insensitively() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let backend = MockGitBackend::new();
        backend
            .set_changed_files(|| async { Ok(vec![changed_file("/path/to/repo1/src/main.rs")]) });
        backend.set_list_labels(|| async { Ok(vec!["Core-Impact".to_string()]) });
        let add_calls = Arc::new(AtomicU32::new(0));
        let ac = Arc::clone(&add_calls);
        backend.set_add_labels(move || {
            let ac = Arc::clone(&ac);
            async move {
                ac.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        });
        let bofa = Bofa::new(config_with_triage())
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.triage_pr(42).await.unwrap();
        assert_eq!(output.labels_applied, vec!["Core-Impact".to_string()]);
        assert!(output.labels_missing.is_empty());
        assert_eq!(add_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn triage_pr_skips_missing_labels_and_applies_existing() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let backend = MockGitBackend::new();
        backend.set_changed_files(|| async {
            Ok(vec![
                changed_file("/path/to/repo1/src/main.rs"),
                changed_file("/docs/README.md"),
            ])
        });
        backend.set_list_labels(|| async { Ok(vec!["core-impact".to_string()]) });
        let add_calls = Arc::new(AtomicU32::new(0));
        let ac = Arc::clone(&add_calls);
        backend.set_add_labels(move || {
            let ac = Arc::clone(&ac);
            async move {
                ac.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        });
        let bofa = Bofa::new(config_with_triage())
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.triage_pr(42).await.unwrap();
        assert_eq!(output.labels_applied, vec!["core-impact".to_string()]);
        assert_eq!(output.labels_missing, vec!["docs".to_string()]);
        assert_eq!(add_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn triage_pr_dry_run_blocks_add_labels() {
        let mut config = config_with_triage();
        config.worker.dry_run = true;
        let backend = MockGitBackend::new();
        backend
            .set_changed_files(|| async { Ok(vec![changed_file("/path/to/repo1/src/main.rs")]) });
        backend.set_list_labels(|| async { Ok(vec!["core-impact".to_string()]) });
        let bofa = Bofa::new(config)
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let err = bofa.triage_pr(42).await.unwrap_err();
        assert!(matches!(err, Error::Git(GitError::DryRun(action)) if action == "add_labels"));
    }

    #[tokio::test]
    async fn triage_pr_skips_update_when_comment_unchanged() {
        use crate::scanner::triage::TriageScanner;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let mut config = config_with_triage();
        config.scanner.triage.post_comment = true;
        let scanner = TriageScanner::new(&config.scanner.triage).unwrap();
        let findings = scanner.scan(&[changed_file("/path/to/repo1/src/main.rs")]);
        let rendered = triage::pr::PrTriageResult {
            findings,
            footnote_template: None,
            app_name: Some("octocat".to_string()),
        }
        .render()
        .unwrap()
        .unwrap();

        let backend = MockGitBackend::new();
        backend
            .set_changed_files(|| async { Ok(vec![changed_file("/path/to/repo1/src/main.rs")]) });
        backend.set_list_comments(move || {
            let rendered = rendered.clone();
            async move { Ok(vec![triage_marked_comment(7, "octocat", &rendered)]) }
        });
        backend.set_list_labels(|| async { Ok(vec!["core-impact".to_string()]) });
        let post_calls = Arc::new(AtomicU32::new(0));
        let update_calls = Arc::new(AtomicU32::new(0));
        let pc = Arc::clone(&post_calls);
        backend.set_post_comment(move || {
            let pc = Arc::clone(&pc);
            async move {
                pc.fetch_add(1, Ordering::SeqCst);
                Ok("https://created".to_string())
            }
        });
        let uc = Arc::clone(&update_calls);
        backend.set_update_comment(move || {
            let uc = Arc::clone(&uc);
            async move {
                uc.fetch_add(1, Ordering::SeqCst);
                Ok("https://updated".to_string())
            }
        });
        let bofa = Bofa::new(config)
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.triage_pr(42).await.unwrap();
        assert_eq!(output.status, triage::pr::CommentStatus::Unchanged);
        assert_eq!(
            output.comment_url,
            Some("https://github.com/owner/repo/pull/42#issuecomment-7".to_string())
        );
        assert_eq!(post_calls.load(Ordering::SeqCst), 0);
        assert_eq!(update_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn triage_pr_updates_existing_marked_comment_when_content_changes() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        let mut config = config_with_triage();
        config.scanner.triage.post_comment = true;
        let backend = MockGitBackend::new();
        backend
            .set_changed_files(|| async { Ok(vec![changed_file("/path/to/repo1/src/main.rs")]) });
        backend.set_list_comments(|| async {
            Ok(vec![triage_marked_comment(7, "octocat", "stale report")])
        });
        backend.set_list_labels(|| async { Ok(vec!["core-impact".to_string()]) });
        let post_calls = Arc::new(AtomicU32::new(0));
        let pc = Arc::clone(&post_calls);
        backend.set_post_comment(move || {
            let pc = Arc::clone(&pc);
            async move {
                pc.fetch_add(1, Ordering::SeqCst);
                Ok("https://created".to_string())
            }
        });
        backend.set_update_comment(|| async {
            Ok("https://github.com/owner/repo/pull/42#issuecomment-7".to_string())
        });
        let bofa = Bofa::new(config)
            .authenticate_with(Box::new(backend))
            .await
            .unwrap();
        let output = bofa.triage_pr(42).await.unwrap();
        assert_eq!(output.status, triage::pr::CommentStatus::Updated);
        assert_eq!(
            output.comment_url,
            Some("https://github.com/owner/repo/pull/42#issuecomment-7".to_string())
        );
        assert_eq!(post_calls.load(Ordering::SeqCst), 0);
    }
}
