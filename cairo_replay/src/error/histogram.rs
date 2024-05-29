//! This file contains the enum `Error` for all the errors returned by the
//! module `histogram`.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// The `Unknown` variant is for any other uncategorised error.
    #[error("Unknown Error generating libfunc histogram: {0:?}")]
    Unknown(String),
}
