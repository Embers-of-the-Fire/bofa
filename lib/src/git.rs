pub mod backend;
pub mod context;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("missing environment variable: {0}")]
    MissingSecret(String),
    #[error("authentication failed: {0}")]
    Authentication(String),
    #[error("API error: {0}")]
    Api(String),
    #[error("unsupported provider: {0}")]
    UnsupportedProvider(String),
    #[error("unsupported operation: {0}")]
    Unsupported(String),
    #[error("dry run blocked non-fetch action: {0}")]
    DryRun(String),
}

#[derive(Debug, Clone)]
pub enum AccountType {
    User,
    Organization,
    Bot,
    GitHubApp,
    Other(String),
}

impl std::fmt::Display for AccountType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccountType::User => write!(f, "user"),
            AccountType::Organization => write!(f, "organization"),
            AccountType::Bot => write!(f, "bot"),
            AccountType::GitHubApp => write!(f, "GitHub App"),
            AccountType::Other(s) => write!(f, "{s}"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AccountMetadata {
    pub id: u64,
    pub login: String,
    pub account_type: AccountType,
    pub installation: Option<Box<AccountMetadata>>,
}

#[derive(Debug, Clone)]
pub struct PullRequestMetadata {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub author: String,
    pub draft: bool,
    pub url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileChangeStatus {
    Added,
    Modified,
    Removed,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct ChangedFile {
    pub path: String,
    pub status: FileChangeStatus,
}
