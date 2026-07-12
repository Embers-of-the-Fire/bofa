use crate::git::{AccountMetadata, AccountType, Error as GitError};
use async_trait::async_trait;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;

type AccountMetadataFuture =
    Pin<Box<dyn Future<Output = Result<AccountMetadata, GitError>> + Send>>;

pub struct MockGitBackend {
    account_metadata_fn: Mutex<Box<dyn Fn() -> AccountMetadataFuture + Send + Sync>>,
    account_metadata_calls: Mutex<u32>,
}

impl Default for MockGitBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl MockGitBackend {
    pub fn new() -> Self {
        Self::with_account_metadata(|| async {
            Ok(AccountMetadata {
                id: 42,
                login: "octocat".to_string(),
                account_type: AccountType::User,
                installation: None,
            })
        })
    }

    pub fn with_account_metadata<F, Fut>(f: F) -> Self
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<AccountMetadata, GitError>> + Send + 'static,
    {
        Self {
            account_metadata_fn: Mutex::new(Box::new(move || Box::pin(f()))),
            account_metadata_calls: Mutex::new(0),
        }
    }

    pub fn set_account_metadata<F, Fut>(&self, f: F)
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<AccountMetadata, GitError>> + Send + 'static,
    {
        *self.account_metadata_fn.lock().unwrap() = Box::new(move || Box::pin(f()));
    }

    pub fn account_metadata_calls(&self) -> u32 {
        *self.account_metadata_calls.lock().unwrap()
    }
}

#[async_trait]
impl super::GitBackend for MockGitBackend {
    async fn account_metadata(&self) -> Result<AccountMetadata, GitError> {
        *self.account_metadata_calls.lock().unwrap() += 1;
        let fut = self.account_metadata_fn.lock().unwrap()();
        fut.await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::backend::GitBackend;

    #[tokio::test]
    async fn returns_default_metadata() {
        let backend = MockGitBackend::new();
        let metadata = backend.account_metadata().await.unwrap();
        assert_eq!(metadata.id, 42);
        assert_eq!(metadata.login, "octocat");
        assert!(matches!(metadata.account_type, AccountType::User));
    }

    #[tokio::test]
    async fn returns_custom_metadata_from_lambda() {
        let backend = MockGitBackend::with_account_metadata(|| async {
            Ok(AccountMetadata {
                id: 1,
                login: "test-org".to_string(),
                account_type: AccountType::Organization,
                installation: None,
            })
        });
        let metadata = backend.account_metadata().await.unwrap();
        assert_eq!(metadata.id, 1);
        assert_eq!(metadata.login, "test-org");
        assert!(matches!(metadata.account_type, AccountType::Organization));
    }

    #[tokio::test]
    async fn returns_error_from_lambda() {
        let backend = MockGitBackend::with_account_metadata(|| async {
            Err(GitError::Api("boom".to_string()))
        });
        let err = backend.account_metadata().await.unwrap_err();
        assert!(matches!(err, GitError::Api(_)));
    }

    #[tokio::test]
    async fn can_reconfigure_after_construction() {
        let backend = MockGitBackend::new();
        backend.set_account_metadata(|| async {
            Ok(AccountMetadata {
                id: 99,
                login: "reconfigured".to_string(),
                account_type: AccountType::Bot,
                installation: None,
            })
        });
        let metadata = backend.account_metadata().await.unwrap();
        assert_eq!(metadata.id, 99);
    }

    #[tokio::test]
    async fn counts_calls() {
        let backend = MockGitBackend::new();
        assert_eq!(backend.account_metadata_calls(), 0);
        backend.account_metadata().await.unwrap();
        assert_eq!(backend.account_metadata_calls(), 1);
        backend.account_metadata().await.unwrap();
        assert_eq!(backend.account_metadata_calls(), 2);
    }
}
