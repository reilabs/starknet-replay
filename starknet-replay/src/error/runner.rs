//! This file contains the enum `Error` for all the errors returned by the
//! module `runner`.

use std::num::TryFromIntError;

use pathfinder_executor::TransactionExecutionError;
use thiserror::Error;

use crate::error::DatabaseError;

#[derive(Debug, Error)]
pub enum Error {
    /// `PathfinderExecutor` is for errors reported by the crate
    /// `pathfinder_executor`.
    #[error(transparent)]
    PathfinderExecutor(#[from] TransactionExecutionError),

    /// `GenerateReplayWork` is used to encapsulate errors of type
    /// `anyhow::Error` which are originating from the
    /// function `starknet_replay::generate_replay_work`.
    #[error(transparent)]
    GenerateReplayWork(anyhow::Error),

    /// `ReplayBlocks` is used to encapsulate errors of type
    /// `anyhow::Error` which are originating from the
    /// function `starknet_replay::replay_blocks`.
    #[error(transparent)]
    ReplayBlocks(anyhow::Error),

    /// `ExecuteBlock` is used to encapsulate errors of type
    /// `anyhow::Error` which are originating from the
    /// function `starknet_replay::execute_block`.
    #[error(transparent)]
    ExecuteBlock(anyhow::Error),

    /// `DatabaseAccess` is used to convert from `DatabaseError` into
    /// `RunnerError` when database functions are called in the module `runner`.
    #[error(transparent)]
    DatabaseAccess(#[from] DatabaseError),

    /// `InsufficientBlocks` is triggered when the most recent block in the
    /// database is less than the starting block of the replay. For obvious
    /// reasons the tool can't continue.
    #[error(
        "Most recent block found in the databse is {last_block}. Exiting because less than \
         start_block {start_block}"
    )]
    InsufficientBlocks { last_block: u64, start_block: u64 },

    /// `CastError` variant is triggered when casting from `u64` to `usize`.
    #[error(transparent)]
    CastError(#[from] TryFromIntError),

    /// `BlockNumberNotValid` variant is triggered when constructing a new
    /// `pathfinder_common::BlockNumber` returns `None`.
    #[error("Block number {block_number} doesn't fit in i64 type.")]
    BlockNumberNotValid { block_number: u64 },

    /// `BlockNotFound` variant is returned when the block requested from the
    /// Pathfinder database isn't found.
    #[error("Block number {block_number} not found in database.")]
    BlockNotFound { block_number: u64 },

    /// The `Unknown` variant is for any other uncategorised error.
    #[error("Unknown Error during block replay: {0:?}")]
    Unknown(String),
}
