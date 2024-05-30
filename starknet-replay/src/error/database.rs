//! This file contains the enum `Error` for all the errors returned by the
//! module `pathfinder_db`.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// `ConnectToDatabase` is used to encapsulate errors of type
    /// `anyhow::Error` which are originating from the
    /// function `starknet_replay::pathfinder_db::connect_to_database`.
    #[error(transparent)]
    ConnectToDatabase(anyhow::Error),

    /// `GetLatestBlockNumber` is used to encapsulate errors of type
    /// `anyhow::Error` which are originating from the
    /// function `starknet_replay::pathfinder_db::get_latest_block_number`.
    #[error(transparent)]
    GetLatestBlockNumber(anyhow::Error),

    /// The `Unknown` variant is for any other uncategorised error.
    #[error("Unknown Error communicating with Pathfinder database: {0:?}")]
    Unknown(String),
}
