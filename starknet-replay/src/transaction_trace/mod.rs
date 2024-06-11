//! This module contains the `starknet-replay` definition of transaction trace
//! structure.

use std::collections::HashMap;

use pathfinder_executor::types::TransactionTrace as PathfinderTransactionTrace;
use starknet_api::core::ClassHash as StarknetClassHash;

use crate::block_number::BlockNumber;

/// The `TransactionTrace` object within `starknet-replay`.
pub struct TransactionTrace {
    /// The block number which the transaction belongs to.
    ///
    /// This is needed to easily query the contract class in the profiler.
    pub block_number: BlockNumber,

    /// The transaction trace object.
    pub trace: PathfinderTransactionTrace,
}
impl TransactionTrace {
    /// This function constructs a new [`TransactionTrace`] object.
    ///
    /// # Arguments
    ///
    /// - `block_number`: The number of the block that `transaction_trace`
    ///   belongs to.
    /// - `transaction_trace`: The transaction trace object.
    #[must_use]
    pub fn new(block_number: BlockNumber, transaction_trace: PathfinderTransactionTrace) -> Self {
        TransactionTrace {
            block_number,
            trace: transaction_trace,
        }
    }

    /// Returns the hashmap of visited program counters for the input `trace`.
    ///
    /// The result of `get_visited_program_counters` is a hashmap where the key
    /// is the [`StarknetClassHash`] and the value is the Vector of visited
    /// program counters for each [`StarknetClassHash`] execution in `trace`.
    ///
    /// If `trace` is not an Invoke transaction, the function returns None
    /// because no libfuncs have been called during the transaction
    /// execution.
    ///
    /// # Arguments
    ///
    /// - `trace`: the [`pathfinder_executor::types::TransactionTrace`] to
    ///   extract the visited program counters from.
    #[must_use]
    pub fn get_visited_program_counters(
        &self,
    ) -> Option<&HashMap<StarknetClassHash, Vec<Vec<usize>>>> {
        match &self.trace {
            PathfinderTransactionTrace::Invoke(tx) => Some(&tx.visited_pcs),
            _ => None,
        }
    }
}
