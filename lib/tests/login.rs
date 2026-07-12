use bofa_lib::action::Bofa;
use bofa_lib::config::credentials::{Credentials, PersonalTokenCredentials, SecretString};
use bofa_lib::config::repository::RepositoryConfig;
use bofa_lib::config::{BofaConfig, Provider};
use bofa_lib::git::backend::mock::MockGitBackend;
use bofa_lib::git::{AccountMetadata, AccountType};

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

#[tokio::test]
async fn login_with_user_account() {
    let backend = Box::new(MockGitBackend::new());
    let bofa = Bofa::new(test_config())
        .authenticate_with(backend)
        .await
        .unwrap();
    let message = bofa.login().await.unwrap();
    assert!(message.contains("Logged in as octocat"));
    assert!(message.contains("42"));
}

#[tokio::test]
async fn login_with_github_app() {
    let backend = Box::new(MockGitBackend::with_account_metadata(|| async {
        Ok(AccountMetadata {
            id: 123,
            login: "test-app".to_string(),
            account_type: AccountType::GitHubApp,
            installation: Some(Box::new(AccountMetadata {
                id: 456,
                login: "test-org".to_string(),
                account_type: AccountType::Organization,
                installation: None,
            })),
        })
    }));
    let bofa = Bofa::new(test_config())
        .authenticate_with(backend)
        .await
        .unwrap();
    let message = bofa.login().await.unwrap();
    assert!(
        message.contains("Logged in as test-app (GitHub App) installed on test-org (organization)")
    );
}

#[tokio::test]
async fn login_with_organization_account() {
    let backend = Box::new(MockGitBackend::with_account_metadata(|| async {
        Ok(AccountMetadata {
            id: 999,
            login: "my-org".to_string(),
            account_type: AccountType::Organization,
            installation: None,
        })
    }));
    let bofa = Bofa::new(test_config())
        .authenticate_with(backend)
        .await
        .unwrap();
    let message = bofa.login().await.unwrap();
    assert!(message.contains("Logged in as my-org (organization), id: 999"));
}
