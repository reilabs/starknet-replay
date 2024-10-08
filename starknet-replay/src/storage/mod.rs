//! The [`Storage`] trait contains the interface between a node storage layer
//! and `starknet-replay`. Implementing this trait allows adding compatibility
//! with a new `Starknet` node.

use std::path::PathBuf;

use starknet_api::block::BlockHeader;
use starknet_api::transaction::{Transaction, TransactionReceipt};
use starknet_core::types::ContractClass;

use crate::block_number::BlockNumber;
use crate::error::DatabaseError;
use crate::runner::replay_class_hash::{ReplayClassHash, TransactionOutput};
use crate::{ReplayBlock, RunnerError};

pub mod rpc;

/// The type [`BlockWithReceipts`] bundles together all the block data: block
/// header, transaction data and receipt data.
pub type BlockWithReceipts = (BlockHeader, Vec<Transaction>, Vec<TransactionReceipt>);

pub trait Storage {
    /// Returns the most recent block number available in the storage.
    ///
    /// If no block is found, it returns 0.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if the low level API with the storage returns an error.
    fn get_most_recent_block_number(&self) -> Result<BlockNumber, DatabaseError>;

    /// Returns the [`starknet_core::types::ContractClass`] object of a
    /// `class_hash`.
    ///
    /// # Arguments
    ///
    /// - `replay_class_hash`: The class hash of the
    ///   [`starknet_core::types::ContractClass`] to return.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if `class_hash` doesn't exist at block `block_num`.
    fn get_contract_class_at_block(
        &self,
        replay_class_hash: &ReplayClassHash,
    ) -> Result<ContractClass, DatabaseError>;

    /// Returns the header of a block.
    ///
    /// # Arguments
    ///
    /// - `block_number`: The block to query.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if `block_id` doesn't exist.
    fn get_block_header(&self, block_number: BlockNumber) -> Result<BlockHeader, DatabaseError>;

    /// Returns the transactions and transaction receipts of a block.
    ///
    /// # Arguments
    ///
    /// - `block_number`: The block to query.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if `block_id` doesn't exist or there are no
    /// transactions.
    fn get_transactions_and_receipts_for_block(
        &self,
        block_number: BlockNumber,
    ) -> Result<BlockWithReceipts, DatabaseError>;

    /// Replays the list of transactions in a block and returns the list of
    /// transactions traces.
    ///
    /// # Arguments
    ///
    /// - `work`: The block to be re-executed.
    /// - `trace_out`: The output file of the transaction trace.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if any transaction fails execution or if there is any
    /// error communicating with the storage layer.
    fn execute_block(
        &self,
        work: &ReplayBlock,
        trace_out: &Option<PathBuf>,
    ) -> Result<Vec<TransactionOutput>, RunnerError>;
}
