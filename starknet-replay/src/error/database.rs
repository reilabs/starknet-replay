//! This file contains the enum `Error` for all the errors returned by the
//! module `pathfinder_db`.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// `ConnectToDatabase` is used to encapsulate errors of type
    /// `anyhow::Error` which are originating from the
    /// function `starknet_replay::runner::pathfinder_db::connect_to_database`.
    #[error(transparent)]
    ConnectToDatabase(anyhow::Error),

    /// `GetLatestBlockNumber` is used to encapsulate errors of type
    /// `anyhow::Error` which are originating from the
    /// function `starknet_replay::runner::pathfinder_db::get_latest_block_number`.
    #[error(transparent)]
    GetLatestBlockNumber(anyhow::Error),

    /// `GetContractClassAtBlock` is used to encapsulate errors of type
    /// `anyhow::Error` which are originating from the
    /// function `starknet_replay::runner::pathfinder_id::get_contract_class_at_block`.
    #[error(transparent)]
    GetContractClassAtBlock(anyhow::Error),

    /// `GetChainId` is used to encapsulate errors of type
    /// `anyhow::Error` which are originating from the
    /// function `starknet_replay::runner::pathfinder_db::get_chain_id`.
    #[error(transparent)]
    GetChainId(anyhow::Error),

    /// `InsufficientBlocks` is triggered when the most recent block in the
    /// database is less than the starting block of the replay. For obvious
    /// reasons the tool can't continue.
    #[error(
        "Most recent block found in the databse is {last_block}. Exiting because less than \
         start_block {start_block}"
    )]
    InsufficientBlocks { last_block: u64, start_block: u64 },

    /// The `Unknown` variant is for any other uncategorised error.
    #[error("Unknown Error communicating with Pathfinder database: {0:?}")]
    Unknown(String),
}
