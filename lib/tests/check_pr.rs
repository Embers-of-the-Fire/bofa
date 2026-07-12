use bofa_lib::action::Bofa;
use bofa_lib::config::credentials::{Credentials, PersonalTokenCredentials, SecretString};
use bofa_lib::config::repository::RepositoryConfig;
use bofa_lib::config::scanner::sensitive::{SensitiveScannerConfig, SensitiveScannerItem};
use bofa_lib::config::{BofaConfig, Provider};
use bofa_lib::git::PullRequestMetadata;
use bofa_lib::git::backend::mock::MockGitBackend;
use bofa_lib::git::{ChangedFile, FileChangeStatus};

fn test_config() -> BofaConfig {
    BofaConfig {
        provider: Provider::GitHub,
        credentials: Credentials::PersonalToken(PersonalTokenCredentials {
            token: SecretString::new("$DUMMY_TOKEN"),
        }),
        repository: RepositoryConfig {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
        },
        scanner: Default::default(),
        log: Default::default(),
    }
}

fn config_with_sensitive_scanner() -> BofaConfig {
    let mut config = test_config();
    config.scanner.sensitive = SensitiveScannerConfig {
        enabled: true,
        item: vec![
            SensitiveScannerItem {
                description: "Core repo".to_string(),
                paths: vec!["/path/to/repo1/**".to_string()],
                members: vec!["alice".to_string(), "bob".to_string()],
            },
            SensitiveScannerItem {
                description: "Other".to_string(),
                paths: vec!["/other/**".to_string()],
                members: vec!["carol".to_string()],
            },
        ],
    };
    config
}

fn changed_file(path: &str) -> ChangedFile {
    ChangedFile {
        path: path.to_string(),
        status: FileChangeStatus::Modified,
    }
}

#[tokio::test]
async fn check_pr_returns_short_metadata() {
    let backend = MockGitBackend::new();
    backend.set_pull_request(|| async {
        Ok(PullRequestMetadata {
            number: 42,
            title: "Fix bug".to_string(),
            state: "closed".to_string(),
            author: "alice".to_string(),
            draft: false,
            url: "https://github.com/owner/repo/pull/42".to_string(),
        })
    });
    let bofa = Bofa::new(test_config())
        .authenticate_with(Box::new(backend))
        .await
        .unwrap();
    let output = bofa.check_pr(42).await.unwrap();
    assert_eq!(
        output,
        "#42 Fix bug by alice [closed] https://github.com/owner/repo/pull/42"
    );
}

#[tokio::test]
async fn check_pr_propagates_backend_error() {
    let backend = MockGitBackend::new();
    backend.set_pull_request(|| async { Err(bofa_lib::git::Error::Api("boom".to_string())) });
    let bofa = Bofa::new(test_config())
        .authenticate_with(Box::new(backend))
        .await
        .unwrap();
    let err = bofa.check_pr(42).await.unwrap_err();
    assert!(matches!(
        err,
        bofa_lib::action::Error::Git(bofa_lib::git::Error::Api(_))
    ));
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
    assert!(output.contains("Core repo"));
    assert!(output.contains("/path/to/repo1/src/main.rs"));
    assert!(output.contains("alice"));
    assert!(output.contains("bob"));
    assert!(output.contains("Other"));
    assert!(output.contains("/other/README.md"));
    assert!(output.contains("carol"));
}

#[tokio::test]
async fn check_pr_reports_no_sensitive_files_changed() {
    let backend = MockGitBackend::new();
    backend.set_changed_files(|| async { Ok(vec![changed_file("/unrelated/file.txt")]) });
    let bofa = Bofa::new(config_with_sensitive_scanner())
        .authenticate_with(Box::new(backend))
        .await
        .unwrap();
    let output = bofa.check_pr(42).await.unwrap();
    assert!(output.contains("No sensitive files changed."));
}

#[tokio::test]
async fn check_pr_calls_changed_files_when_scanner_enabled() {
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
    let bofa = Bofa::new(config_with_sensitive_scanner())
        .authenticate_with(Box::new(backend))
        .await
        .unwrap();
    bofa.check_pr(42).await.unwrap();
    assert_eq!(call_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn check_pr_does_not_call_changed_files_when_scanner_disabled() {
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
    let bofa = Bofa::new(test_config())
        .authenticate_with(Box::new(backend))
        .await
        .unwrap();
    bofa.check_pr(42).await.unwrap();
    assert_eq!(call_count.load(Ordering::SeqCst), 0);
}
