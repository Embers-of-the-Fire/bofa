use crate::git::{
    AccountMetadata, AccountType, ChangedFile, Error as GitError, IssueComment, PullRequestMetadata,
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
type PostCommentFuture = Pin<Box<dyn Future<Output = Result<String, GitError>> + Send>>;
type ListCommentsFuture = Pin<Box<dyn Future<Output = Result<Vec<IssueComment>, GitError>> + Send>>;
type UpdateCommentFuture = Pin<Box<dyn Future<Output = Result<String, GitError>> + Send>>;

pub struct MockGitBackend {
    account_metadata_fn: Mutex<Box<dyn Fn() -> AccountMetadataFuture + Send + Sync>>,
    account_metadata_calls: Mutex<u32>,
    pull_request_fn: Mutex<Box<dyn Fn() -> PullRequestFuture + Send + Sync>>,
    pull_request_calls: Mutex<u32>,
    changed_files_fn: Mutex<Box<dyn Fn() -> ChangedFilesFuture + Send + Sync>>,
    changed_files_calls: Mutex<u32>,
    post_comment_fn: Mutex<Box<dyn Fn() -> PostCommentFuture + Send + Sync>>,
    post_comment_calls: Mutex<u32>,
    list_comments_fn: Mutex<Box<dyn Fn() -> ListCommentsFuture + Send + Sync>>,
    list_comments_calls: Mutex<u32>,
    update_comment_fn: Mutex<Box<dyn Fn() -> UpdateCommentFuture + Send + Sync>>,
    update_comment_calls: Mutex<u32>,
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
            post_comment_fn: Mutex::new(Box::new(move || {
                Box::pin(async {
                    Ok("https://github.com/test/repo/pull/1#issuecomment-1".to_string())
                })
            })),
            post_comment_calls: Mutex::new(0),
            list_comments_fn: Mutex::new(Box::new(move || Box::pin(async { Ok(Vec::new()) }))),
            list_comments_calls: Mutex::new(0),
            update_comment_fn: Mutex::new(Box::new(move || {
                Box::pin(async {
                    Ok("https://github.com/test/repo/pull/1#issuecomment-1".to_string())
                })
            })),
            update_comment_calls: Mutex::new(0),
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

    pub fn set_post_comment<F, Fut>(&self, f: F)
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<String, GitError>> + Send + 'static,
    {
        *self.post_comment_fn.lock().unwrap() = Box::new(move || Box::pin(f()));
    }

    pub fn post_comment_calls(&self) -> u32 {
        *self.post_comment_calls.lock().unwrap()
    }

    pub fn set_list_comments<F, Fut>(&self, f: F)
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<Vec<IssueComment>, GitError>> + Send + 'static,
    {
        *self.list_comments_fn.lock().unwrap() = Box::new(move || Box::pin(f()));
    }

    pub fn list_comments_calls(&self) -> u32 {
        *self.list_comments_calls.lock().unwrap()
    }

    pub fn set_update_comment<F, Fut>(&self, f: F)
    where
        F: Fn() -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<String, GitError>> + Send + 'static,
    {
        *self.update_comment_fn.lock().unwrap() = Box::new(move || Box::pin(f()));
    }

    pub fn update_comment_calls(&self) -> u32 {
        *self.update_comment_calls.lock().unwrap()
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

    async fn post_comment(
        &self,
        _owner: &str,
        _repo: &str,
        _id: u64,
        _body: &str,
    ) -> Result<String, GitError> {
        *self.post_comment_calls.lock().unwrap() += 1;
        let fut = self.post_comment_fn.lock().unwrap()();
        fut.await
    }

    async fn list_comments(
        &self,
        _owner: &str,
        _repo: &str,
        _id: u64,
    ) -> Result<Vec<IssueComment>, GitError> {
        *self.list_comments_calls.lock().unwrap() += 1;
        let fut = self.list_comments_fn.lock().unwrap()();
        fut.await
    }

    async fn update_comment(
        &self,
        _owner: &str,
        _repo: &str,
        _comment_id: u64,
        _body: &str,
    ) -> Result<String, GitError> {
        *self.update_comment_calls.lock().unwrap() += 1;
        let fut = self.update_comment_fn.lock().unwrap()();
        fut.await
    }

    async fn delete_branch(
        &self,
        _owner: &str,
        _repo: &str,
        _branch: &str,
    ) -> Result<(), GitError> {
        Err(GitError::Unsupported("delete_branch".to_string()))
    }

    async fn publish_release(&self, _owner: &str, _repo: &str, _tag: &str) -> Result<(), GitError> {
        Err(GitError::Unsupported("publish_release".to_string()))
    }

    async fn upload_file(
        &self,
        _owner: &str,
        _repo: &str,
        _path: &str,
        _content: &[u8],
    ) -> Result<(), GitError> {
        Err(GitError::Unsupported("upload_file".to_string()))
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

    #[tokio::test]
    async fn returns_default_post_comment_url() {
        let backend = MockGitBackend::new();
        let url = backend
            .post_comment("owner", "repo", 1, "body")
            .await
            .unwrap();
        assert_eq!(url, "https://github.com/test/repo/pull/1#issuecomment-1");
    }

    #[tokio::test]
    async fn returns_custom_post_comment_url_from_lambda() {
        let backend = MockGitBackend::new();
        backend.set_post_comment(|| async {
            Ok("https://github.com/custom/repo/pull/42#issuecomment-42".to_string())
        });
        let url = backend
            .post_comment("owner", "repo", 1, "body")
            .await
            .unwrap();
        assert_eq!(
            url,
            "https://github.com/custom/repo/pull/42#issuecomment-42"
        );
    }

    #[tokio::test]
    async fn returns_post_comment_error_from_lambda() {
        let backend = MockGitBackend::new();
        backend.set_post_comment(|| async { Err(GitError::Api("boom".to_string())) });
        let err = backend
            .post_comment("owner", "repo", 1, "body")
            .await
            .unwrap_err();
        assert!(matches!(err, GitError::Api(_)));
    }

    #[tokio::test]
    async fn counts_post_comment_calls() {
        let backend = MockGitBackend::new();
        assert_eq!(backend.post_comment_calls(), 0);
        backend
            .post_comment("owner", "repo", 1, "body")
            .await
            .unwrap();
        assert_eq!(backend.post_comment_calls(), 1);
        backend
            .post_comment("owner", "repo", 1, "body")
            .await
            .unwrap();
        assert_eq!(backend.post_comment_calls(), 2);
    }

    #[tokio::test]
    async fn returns_default_empty_list_comments() {
        let backend = MockGitBackend::new();
        let comments = backend.list_comments("owner", "repo", 1).await.unwrap();
        assert!(comments.is_empty());
    }

    #[tokio::test]
    async fn returns_custom_list_comments_from_lambda() {
        let backend = MockGitBackend::new();
        backend.set_list_comments(|| async {
            Ok(vec![IssueComment {
                id: 7,
                body: "hello".to_string(),
                author_login: "octocat".to_string(),
                url: "https://github.com/test/repo/pull/1#issuecomment-7".to_string(),
            }])
        });
        let comments = backend.list_comments("owner", "repo", 1).await.unwrap();
        assert_eq!(comments.len(), 1);
        assert_eq!(comments[0].id, 7);
        assert_eq!(comments[0].author_login, "octocat");
    }

    #[tokio::test]
    async fn returns_list_comments_error_from_lambda() {
        let backend = MockGitBackend::new();
        backend.set_list_comments(|| async { Err(GitError::Api("boom".to_string())) });
        let err = backend.list_comments("owner", "repo", 1).await.unwrap_err();
        assert!(matches!(err, GitError::Api(_)));
    }

    #[tokio::test]
    async fn counts_list_comments_calls() {
        let backend = MockGitBackend::new();
        assert_eq!(backend.list_comments_calls(), 0);
        backend.list_comments("owner", "repo", 1).await.unwrap();
        assert_eq!(backend.list_comments_calls(), 1);
    }

    #[tokio::test]
    async fn returns_default_update_comment_url() {
        let backend = MockGitBackend::new();
        let url = backend
            .update_comment("owner", "repo", 7, "body")
            .await
            .unwrap();
        assert_eq!(url, "https://github.com/test/repo/pull/1#issuecomment-1");
    }

    #[tokio::test]
    async fn returns_custom_update_comment_url_from_lambda() {
        let backend = MockGitBackend::new();
        backend.set_update_comment(|| async {
            Ok("https://github.com/custom/repo/pull/42#issuecomment-99".to_string())
        });
        let url = backend
            .update_comment("owner", "repo", 7, "body")
            .await
            .unwrap();
        assert_eq!(
            url,
            "https://github.com/custom/repo/pull/42#issuecomment-99"
        );
    }

    #[tokio::test]
    async fn returns_update_comment_error_from_lambda() {
        let backend = MockGitBackend::new();
        backend.set_update_comment(|| async { Err(GitError::Api("boom".to_string())) });
        let err = backend
            .update_comment("owner", "repo", 7, "body")
            .await
            .unwrap_err();
        assert!(matches!(err, GitError::Api(_)));
    }

    #[tokio::test]
    async fn counts_update_comment_calls() {
        let backend = MockGitBackend::new();
        assert_eq!(backend.update_comment_calls(), 0);
        backend
            .update_comment("owner", "repo", 7, "body")
            .await
            .unwrap();
        assert_eq!(backend.update_comment_calls(), 1);
    }
}
