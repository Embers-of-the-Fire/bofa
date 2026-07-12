use tracing::{instrument, trace};

pub struct GitContext {
    backend: Box<dyn super::backend::GitBackend>,
}

impl GitContext {
    pub fn from_backend(backend: Box<dyn super::backend::GitBackend>) -> Self {
        trace!("creating git context from backend");
        Self { backend }
    }

    #[instrument(skip(self), err)]
    pub async fn account_metadata(&self) -> Result<super::AccountMetadata, super::Error> {
        self.backend.account_metadata().await
    }

    #[instrument(skip(self), fields(owner, repo, id), err)]
    pub async fn pull_request(
        &self,
        owner: &str,
        repo: &str,
        id: u64,
    ) -> Result<super::PullRequestMetadata, super::Error> {
        self.backend.pull_request(owner, repo, id).await
    }

    #[instrument(skip(self), fields(owner, repo, id), err)]
    pub async fn changed_files(
        &self,
        owner: &str,
        repo: &str,
        id: u64,
    ) -> Result<Vec<super::ChangedFile>, super::Error> {
        self.backend.changed_files(owner, repo, id).await
    }

    #[instrument(skip(self), fields(owner, repo, branch), err)]
    pub async fn delete_branch(
        &self,
        owner: &str,
        repo: &str,
        branch: &str,
    ) -> Result<(), super::Error> {
        self.backend.delete_branch(owner, repo, branch).await
    }

    #[instrument(skip(self), fields(owner, repo, tag), err)]
    pub async fn publish_release(
        &self,
        owner: &str,
        repo: &str,
        tag: &str,
    ) -> Result<(), super::Error> {
        self.backend.publish_release(owner, repo, tag).await
    }

    #[instrument(skip(self, content), fields(owner, repo, path), err)]
    pub async fn upload_file(
        &self,
        owner: &str,
        repo: &str,
        path: &str,
        content: &[u8],
    ) -> Result<(), super::Error> {
        self.backend.upload_file(owner, repo, path, content).await
    }
}
