//! This file contains the enum `Error` for all the errors returned by the
//! module `runner`.

use std::num::TryFromIntError;

use pathfinder_executor::TransactionExecutionError;
use thiserror::Error;

use crate::block_number::BlockNumber;
use crate::error::DatabaseError;

#[derive(Debug, Error)]
pub enum Error {
    /// `PathfinderExecutor` is for errors reported by the crate
    /// [`pathfinder_executor`].
    #[error(transparent)]
    PathfinderExecutor(#[from] TransactionExecutionError),

    /// `GenerateReplayWork` is used to encapsulate errors of type
    /// [`anyhow::Error`] which are originating from the function
    /// [`crate::runner::generate_replay_work`].
    #[error(transparent)]
    GenerateReplayWork(anyhow::Error),

    /// `ReplayBlocks` is used to encapsulate errors of type [`anyhow::Error`]
    /// which are originating from the function
    /// [`crate::runner::generate_replay_work`].
    #[error(transparent)]
    ReplayBlocks(anyhow::Error),

    /// `ExecuteBlock` is used to encapsulate errors of type [`anyhow::Error`]
    /// which are originating from the function
    /// [`crate::runner::generate_replay_work`].
    #[error(transparent)]
    ExecuteBlock(anyhow::Error),

    /// `DatabaseAccess` is used to report [`crate::error::DatabaseError`] when
    /// database functions are called in the module [`crate::runner`].
    #[error(transparent)]
    DatabaseAccess(#[from] DatabaseError),

    /// `InsufficientBlocks` is triggered when the most recent block in the
    /// database is less than the starting block of the replay. For obvious
    /// reasons the tool can't continue.
    #[error(
        "Most recent block found in the databse is {last_block}. Exiting because less than \
         start_block {start_block}"
    )]
    InsufficientBlocks {
        last_block: BlockNumber,
        start_block: BlockNumber,
    },

    /// `CastError` variant is triggered when casting from `u64` to `usize`.
    #[error(transparent)]
    CastError(#[from] TryFromIntError),

    /// `BlockNumberNotValid` variant is triggered when constructing a new
    /// [`crate::block_number::BlockNumber`] returns `None`.
    #[error("Block number {block_number} doesn't fit in i64 type.")]
    BlockNumberNotValid { block_number: u64 },

    /// The `Unknown` variant is for any other uncategorised error.
    #[error("Unknown Error during block replay: {0:?}")]
    Unknown(String),
}
