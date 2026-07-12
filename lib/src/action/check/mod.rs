pub mod pr;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid pull request identifier: {0}")]
    InvalidIdentifier(String),
}
