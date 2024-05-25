use anyhow::bail;
use cairo_lang_utils::ordered_hash_map::OrderedHashMap;
use pathfinder_common::receipt::Receipt;
use pathfinder_common::transaction::Transaction as StarknetTransaction;
use pathfinder_common::BlockHeader;
use smol_str::SmolStr;

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
    /// The key corresponds to the concrete libfunc name and the value
    /// contains the number of times the libfunc has been called
    /// during execution of all the transactions in the block
    pub libfuncs_weight: OrderedHashMap<SmolStr, usize>,
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
    ) -> anyhow::Result<ReplayBlock> {
        if transactions.len() != receipts.len() {
            bail!(
                "The length of `transactions` must match the length of \
                 `receipts` to create a new `ReplayBlock` struct."
            )
        }
        Ok(Self {
            header,
            transactions,
            receipts,
            libfuncs_weight: OrderedHashMap::default(),
        })
    }

    /// Update `libfuncs_weight` from the input `libfuncs_weight`
    ///
    /// Data in `libfuncs_weight` is used to update the cumulative block
    /// statistics on the usage of libfuncs.
    ///
    /// # Arguments
    ///
    /// - `libfuncs_weight`: The input hashmap to update `self.libfuncs_weight`
    pub fn add_libfuncs(
        &mut self,
        libfuncs_weight: &OrderedHashMap<SmolStr, usize>,
    ) {
        for (libfunc, weight) in libfuncs_weight.iter() {
            self.libfuncs_weight
                .entry(libfunc.clone())
                .and_modify(|e| *e += *weight)
                .or_insert(*weight);
        }
    }

    /// `libfuncs_weight` is updated with data from `self.libfuncs_weight`.
    ///
    /// The reverse of `self.add_libfuncs`.
    ///
    /// # Arguments
    ///
    /// - `libfuncs_weight`: The output hashmap to update with data in
    ///   `self.libfuncs_weight`
    pub fn extend_libfunc_stats(
        &self,
        libfuncs_weight: &mut OrderedHashMap<SmolStr, usize>,
    ) {
        for (libfunc, weight) in self.libfuncs_weight.iter() {
            libfuncs_weight
                .entry(libfunc.clone())
                .and_modify(|e| *e += *weight)
                .or_insert(*weight);
        }
    }
}
