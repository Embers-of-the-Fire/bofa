use super::GitBackend;
use async_trait::async_trait;
use tracing::warn;

pub struct DryRunBackend {
    inner: Box<dyn GitBackend>,
}

impl DryRunBackend {
    pub fn new(inner: Box<dyn GitBackend>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl GitBackend for DryRunBackend {
    async fn account_metadata(&self) -> Result<super::super::AccountMetadata, super::super::Error> {
        self.inner.account_metadata().await
    }

    async fn pull_request(
        &self,
        owner: &str,
        repo: &str,
        id: u64,
    ) -> Result<super::super::PullRequestMetadata, super::super::Error> {
        self.inner.pull_request(owner, repo, id).await
    }

    async fn changed_files(
        &self,
        owner: &str,
        repo: &str,
        id: u64,
    ) -> Result<Vec<super::super::ChangedFile>, super::super::Error> {
        self.inner.changed_files(owner, repo, id).await
    }

    async fn post_comment(
        &self,
        _owner: &str,
        _repo: &str,
        _id: u64,
        _body: &str,
    ) -> Result<String, super::super::Error> {
        warn!(action = "post_comment", "dry run blocked mutating action");
        Err(super::super::Error::DryRun("post_comment".to_string()))
    }

    async fn delete_branch(
        &self,
        _owner: &str,
        _repo: &str,
        _branch: &str,
    ) -> Result<(), super::super::Error> {
        warn!(action = "delete_branch", "dry run blocked mutating action");
        Err(super::super::Error::DryRun("delete_branch".to_string()))
    }

    async fn publish_release(
        &self,
        _owner: &str,
        _repo: &str,
        _tag: &str,
    ) -> Result<(), super::super::Error> {
        warn!(
            action = "publish_release",
            "dry run blocked mutating action"
        );
        Err(super::super::Error::DryRun("publish_release".to_string()))
    }

    async fn upload_file(
        &self,
        _owner: &str,
        _repo: &str,
        _path: &str,
        _content: &[u8],
    ) -> Result<(), super::super::Error> {
        warn!(action = "upload_file", "dry run blocked mutating action");
        Err(super::super::Error::DryRun("upload_file".to_string()))
    }
}
