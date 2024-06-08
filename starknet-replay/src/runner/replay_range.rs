//! This module contains the implementation of `ReplayRange` to keep track of
//! the range of blocks to be replayed. This struct also ensures to the user
//! that starting block is not greater than end block.

use crate::common::BlockNumber;
use crate::error::RunnerError;

/// `ReplayRange` contains the block range that is replayed by
/// `starknet-replay`. The fields are not public to ensure no tampering after
/// the struct is initialised.
#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct ReplayRange {
    /// The first block to replay.
    start_block: BlockNumber,

    /// The last block to replay (inclusive).
    end_block: BlockNumber,
}

impl ReplayRange {
    /// Constructs a new `ReplayRange` object.
    ///
    /// The constructor checks that `start_block` is not greater than
    /// `end_block`.
    ///
    /// # Arguments
    ///
    /// - `start_block`: The first block to replay.
    /// - `end_block`: The last block to replay (inclusive).
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if `start_block` is greater than `end_block`.
    pub fn new(start_block: u64, end_block: u64) -> Result<ReplayRange, RunnerError> {
        if start_block > end_block {
            return Err(RunnerError::Unknown(
                "Exiting because end_block must be greater or equal to start_block.".to_string(),
            ));
        }

        Ok(Self {
            start_block: BlockNumber::new(start_block),
            end_block: BlockNumber::new(start_block),
        })
    }

    /// Get `start_block` field of `ReplayRange`.
    #[must_use]
    pub fn get_start_block(&self) -> BlockNumber {
        self.start_block
    }

    /// Get `end_block` field of `ReplayRange`.
    #[must_use]
    pub fn get_end_block(&self) -> BlockNumber {
        self.end_block
    }
}
