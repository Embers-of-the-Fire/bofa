pub mod pr;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid pull request identifier: {0}")]
    InvalidIdentifier(String),
    #[error("scanner error: {0}")]
    Scanner(#[from] crate::scanner::sensitive::Error),
    #[error("template error: {0}")]
    Template(String),
}
