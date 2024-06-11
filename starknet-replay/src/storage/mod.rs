//! The [`Storage`] trait contains the interface between a node storage layer
//! and `starknet-replay`. Implementing this trait allows adding compatibility
//! with a new `Starknet` node.

use pathfinder_common::receipt::Receipt;
use pathfinder_common::transaction::Transaction;
use pathfinder_common::{BlockHeader, ChainId};
use pathfinder_executor::types::TransactionSimulation;
use pathfinder_rpc::v02::types::ContractClass;

use crate::block_number::BlockNumber;
use crate::error::DatabaseError;
use crate::runner::replay_class_hash::ReplayClassHash;
use crate::{ReplayBlock, RunnerError};

pub mod pathfinder;

pub trait Storage {
    /// Returns the most recent block number available in the storage.
    ///
    /// If no block is found, it returns 0.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if the low level API with the storage returns an error.
    fn get_most_recent_block_number(&self) -> Result<BlockNumber, DatabaseError>;

    /// Get the `chain_id` of the storage.
    ///
    /// It can detect only Mainnet, Goerli, and Sepolia.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if:
    ///
    /// - The chain isn't recognised or supported.
    /// - There is an error querying the storage layer.
    fn get_chain_id(&self) -> Result<ChainId, DatabaseError>;

    /// Returns the [`pathfinder_rpc::v02::types::ContractClass`] object of a
    /// `class_hash`.
    ///
    /// # Arguments
    ///
    /// - `replay_class_hash`: The class hash of the
    ///   [`pathfinder_rpc::v02::types::ContractClass`] to return.
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
    ) -> Result<(Vec<Transaction>, Vec<Receipt>), DatabaseError>;

    /// Replays the list of transactions in a block and returns the list of
    /// transactions traces.
    ///
    /// # Arguments
    ///
    /// - `work`: The block to be re-executed
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if any transaction fails execution or if there is any
    /// error communicating with the storage layer.
    fn execute_block(&self, work: &ReplayBlock) -> Result<Vec<TransactionSimulation>, RunnerError>;
}
