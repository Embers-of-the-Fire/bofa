pub mod pr;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid pull request identifier: {0}")]
    InvalidIdentifier(String),
    #[error("scanner error: {0}")]
    Scanner(#[from] crate::scanner::triage::Error),
    #[error("invalid ignore glob pattern: {0}")]
    InvalidIgnoreGlob(String),
    #[error("template error: {0}")]
    Template(String),
}
