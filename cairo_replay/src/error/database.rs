//! This file contains the enum `Error` for all the errors returned by the
//! module `pathfinder_db`.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// This enum variant is used to encapsulate errors of type
    /// `anyhow::Error` which are originating from the
    /// function `crate::pathfinder_db::connect_to_database`.
    #[error(transparent)]
    ConnectToDatabase(anyhow::Error),

    /// This enum variant is used to encapsulate errors of type
    /// `anyhow::Error` which are originating from the
    /// function `crate::pathfinder_db::get_latest_block_number`.
    #[error(transparent)]
    GetLatestBlockNumber(anyhow::Error),

    /// For any other uncategorised error.
    #[error("error communicating with Pathfinder database")]
    Unknown(String),
}
