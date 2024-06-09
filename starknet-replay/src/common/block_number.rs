//! This is `starknet-replay` representation of a `Starknet` block number.

/// `BlockNumber` is represented as a `u64` integer.
#[derive(Copy, Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
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
