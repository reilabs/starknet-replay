//! This is `starknet-replay`s representation of a `Starknet` block number.

use std::fmt;

pub mod pathfinder;

/// `BlockNumber` is represented as a `u64` integer.
#[derive(Copy, Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockNumber(u64);
impl BlockNumber {
    /// Creates a new `BlockNumber`.
    #[must_use]
    pub fn new(block_number: u64) -> Self {
        BlockNumber(block_number)
    }

    /// Returns the block number as `u64`.
    #[must_use]
    pub fn get(&self) -> u64 {
        self.0
    }
}
impl fmt::Display for BlockNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
