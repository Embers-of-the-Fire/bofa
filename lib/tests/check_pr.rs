use bofa_lib::action::Bofa;
use bofa_lib::config::credentials::{Credentials, PersonalTokenCredentials, SecretString};
use bofa_lib::config::repository::RepositoryConfig;
use bofa_lib::config::{BofaConfig, Provider};
use bofa_lib::git::PullRequestMetadata;
use bofa_lib::git::backend::mock::MockGitBackend;

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
