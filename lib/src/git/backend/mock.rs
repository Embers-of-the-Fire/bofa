use crate::git::{
    AccountMetadata, AccountType, ChangedFile, Error as GitError, PullRequestMetadata,
};
use async_trait::async_trait;
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;

type AccountMetadataFuture =
    Pin<Box<dyn Future<Output = Result<AccountMetadata, GitError>> + Send>>;
type PullRequestFuture =
    Pin<Box<dyn Future<Output = Result<PullRequestMetadata, GitError>> + Send>>;
type ChangedFilesFuture = Pin<Box<dyn Future<Output = Result<Vec<ChangedFile>, GitError>> + Send>>;

pub struct MockGitBackend {
    account_metadata_fn: Mutex<Box<dyn Fn() -> AccountMetadataFuture + Send + Sync>>,
    account_metadata_calls: Mutex<u32>,
    pull_request_fn: Mutex<Box<dyn Fn() -> PullRequestFuture + Send + Sync>>,
    pull_request_calls: Mutex<u32>,
    changed_files_fn: Mutex<Box<dyn Fn() -> ChangedFilesFuture + Send + Sync>>,
    changed_files_calls: Mutex<u32>,
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
            pull_request_fn: Mutex::new(Box::new(move || {
                Box::pin(async {
                    Ok(PullRequestMetadata {
                        number: 1,
                        title: "Test PR".to_string(),
                        state: "open".to_string(),
                        author: "octocat".to_string(),
                        draft: false,
                        url: "https://github.com/test/repo/pull/1".to_string(),
                    })
                })
            })),
            pull_request_calls: Mutex::new(0),
            changed_files_fn: Mutex::new(Box::new(move || Box::pin(async { Ok(Vec::new()) }))),
            changed_files_calls: Mutex::new(0),
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

    pub fn set_pull_request<F, Fut>(&self, f: F)
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<PullRequestMetadata, GitError>> + Send + 'static,
    {
        *self.pull_request_fn.lock().unwrap() = Box::new(move || Box::pin(f()));
    }

    pub fn pull_request_calls(&self) -> u32 {
        *self.pull_request_calls.lock().unwrap()
    }

    pub fn set_changed_files<F, Fut>(&self, f: F)
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Vec<ChangedFile>, GitError>> + Send + 'static,
    {
        *self.changed_files_fn.lock().unwrap() = Box::new(move || Box::pin(f()));
    }

    pub fn changed_files_calls(&self) -> u32 {
        *self.changed_files_calls.lock().unwrap()
    }
}

#[async_trait]
impl super::GitBackend for MockGitBackend {
    async fn account_metadata(&self) -> Result<AccountMetadata, GitError> {
        *self.account_metadata_calls.lock().unwrap() += 1;
        let fut = self.account_metadata_fn.lock().unwrap()();
        fut.await
    }

    async fn pull_request(
        &self,
        _owner: &str,
        _repo: &str,
        _id: u64,
    ) -> Result<PullRequestMetadata, GitError> {
        *self.pull_request_calls.lock().unwrap() += 1;
        let fut = self.pull_request_fn.lock().unwrap()();
        fut.await
    }

    async fn changed_files(
        &self,
        _owner: &str,
        _repo: &str,
        _id: u64,
    ) -> Result<Vec<ChangedFile>, GitError> {
        *self.changed_files_calls.lock().unwrap() += 1;
        let fut = self.changed_files_fn.lock().unwrap()();
        fut.await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::FileChangeStatus;
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

    #[tokio::test]
    async fn returns_default_pull_request() {
        let backend = MockGitBackend::new();
        let metadata = backend.pull_request("owner", "repo", 1).await.unwrap();
        assert_eq!(metadata.number, 1);
        assert_eq!(metadata.title, "Test PR");
        assert_eq!(metadata.state, "open");
        assert_eq!(metadata.author, "octocat");
    }

    #[tokio::test]
    async fn returns_custom_pull_request_from_lambda() {
        let backend = MockGitBackend::new();
        backend.set_pull_request(|| async {
            Ok(PullRequestMetadata {
                number: 42,
                title: "Custom PR".to_string(),
                state: "closed".to_string(),
                author: "alice".to_string(),
                draft: false,
                url: "https://github.com/custom/repo/pull/42".to_string(),
            })
        });
        let metadata = backend.pull_request("owner", "repo", 1).await.unwrap();
        assert_eq!(metadata.number, 42);
        assert_eq!(metadata.title, "Custom PR");
        assert_eq!(metadata.state, "closed");
        assert_eq!(metadata.author, "alice");
    }

    #[tokio::test]
    async fn returns_pull_request_error_from_lambda() {
        let backend = MockGitBackend::new();
        backend.set_pull_request(|| async { Err(GitError::Api("boom".to_string())) });
        let err = backend.pull_request("owner", "repo", 1).await.unwrap_err();
        assert!(matches!(err, GitError::Api(_)));
    }

    #[tokio::test]
    async fn counts_pull_request_calls() {
        let backend = MockGitBackend::new();
        assert_eq!(backend.pull_request_calls(), 0);
        backend.pull_request("owner", "repo", 1).await.unwrap();
        assert_eq!(backend.pull_request_calls(), 1);
        backend.pull_request("owner", "repo", 1).await.unwrap();
        assert_eq!(backend.pull_request_calls(), 2);
    }

    #[tokio::test]
    async fn returns_default_changed_files() {
        let backend = MockGitBackend::new();
        let files = backend.changed_files("owner", "repo", 1).await.unwrap();
        assert!(files.is_empty());
    }

    #[tokio::test]
    async fn returns_custom_changed_files_from_lambda() {
        let backend = MockGitBackend::new();
        backend.set_changed_files(|| async {
            Ok(vec![
                ChangedFile {
                    path: "src/main.rs".to_string(),
                    status: FileChangeStatus::Modified,
                },
                ChangedFile {
                    path: "README.md".to_string(),
                    status: FileChangeStatus::Added,
                },
            ])
        });
        let files = backend.changed_files("owner", "repo", 1).await.unwrap();
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "src/main.rs");
        assert_eq!(files[0].status, FileChangeStatus::Modified);
        assert_eq!(files[1].path, "README.md");
        assert_eq!(files[1].status, FileChangeStatus::Added);
    }

    #[tokio::test]
    async fn returns_changed_files_error_from_lambda() {
        let backend = MockGitBackend::new();
        backend.set_changed_files(|| async { Err(GitError::Api("boom".to_string())) });
        let err = backend.changed_files("owner", "repo", 1).await.unwrap_err();
        assert!(matches!(err, GitError::Api(_)));
    }

    #[tokio::test]
    async fn counts_changed_files_calls() {
        let backend = MockGitBackend::new();
        assert_eq!(backend.changed_files_calls(), 0);
        backend.changed_files("owner", "repo", 1).await.unwrap();
        assert_eq!(backend.changed_files_calls(), 1);
        backend.changed_files("owner", "repo", 1).await.unwrap();
        assert_eq!(backend.changed_files_calls(), 2);
    }
}
