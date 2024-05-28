use pathfinder_common::receipt::Receipt;
use pathfinder_common::transaction::Transaction as StarknetTransaction;
use pathfinder_common::BlockHeader;

use crate::error::RunnerError;

/// `ReplayBlock` contains the data necessary to replay a single block from
/// the Starknet blockchain.
#[derive(Debug, Clone, Eq, PartialEq, Default)]
pub struct ReplayBlock {
    /// The header of the block being replayed.
    pub header: BlockHeader,
    /// The list of transactions to be replayed.
    ///
    /// There isn't any check that:
    /// - the transactions belong to block `header`
    /// - there aren't missing transactions from block `header`
    // TODO: analyse if there is a workaround to enforce that transactions
    // aren't misplaced in the wrong block
    pub transactions: Vec<StarknetTransaction>,
    /// The list of receipts of `transactions`.
    ///
    /// The receipt of each transaction in the `transactions` vector is found
    /// at matching index in the `receipts` vector.
    pub receipts: Vec<Receipt>,
}

impl ReplayBlock {
    /// Create a new batch of work to be replayed.
    ///
    /// Not checking that `transactions` and `receipts` have the same length.
    /// The receipt for transaction at index I is found at index I of `receipt`.
    ///
    /// # Arguments
    ///
    /// - `header`: The header of the block that the `transactions` belong to.
    /// - `transactions`: The list of transactions in the block that need to be
    ///   profiled.
    /// - `receipts`: The list of receipts for the execution of the
    ///   transactions. Must be the same length as `transactions`.
    pub fn new(
        header: BlockHeader,
        transactions: Vec<StarknetTransaction>,
        receipts: Vec<Receipt>,
    ) -> Result<ReplayBlock, RunnerError> {
        if transactions.len() != receipts.len() {
            return Err(RunnerError::Unknown(
                "The length of `transactions` must match the length of `receipts` to create a new \
                 `ReplayBlock` struct."
                    .to_string(),
            ));
        }
        Ok(Self {
            header,
            transactions,
            receipts,
        })
    }
}
