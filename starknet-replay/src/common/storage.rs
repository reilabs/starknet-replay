//! The `Storage` trait contains the interface between a node storage layer and
//! `starknet-replay`. Implementing this trait allows to add compatibility with
//! a new `Starknet` node.

use pathfinder_common::receipt::Receipt;
use pathfinder_common::transaction::Transaction;
use pathfinder_common::{BlockHeader, ChainId};
use pathfinder_rpc::v02::types::ContractClass;

use crate::common::BlockNumber;
use crate::error::DatabaseError;
use crate::runner::replay_class_hash::ReplayClassHash;
use crate::runner::VisitedPcs;
use crate::{ReplayBlock, RunnerError};

pub trait Storage {
    /// Returns the latest (most recent) block number in the database
    ///
    /// If no block is found in the database, it returns 0.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if the low level API with the database returns an error.
    fn get_latest_block_number(&self) -> Result<BlockNumber, DatabaseError>;

    /// Get the `chain_id` of the Pathfinder database.
    ///
    /// This function detects the chain used by quering the hash of the first
    /// block in the database. It can detect only Mainnet, Goerli, and
    /// Sepolia.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if:
    ///
    /// - The first block doesn't have a hash matching one of
    /// the known hashes.
    /// - There is an error querying the database.
    fn get_chain_id(&self) -> Result<ChainId, DatabaseError>;

    /// Returns the `ContractClass` object of a `class_hash` at `block_num` from
    /// the Pathfinder database `db`.
    ///
    /// # Arguments
    ///
    /// - `replay_class_hash`: The class hash of the `ContractClass` to return.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if `class_hash` doesn't exist at block `block_num`.
    fn get_contract_class_at_block(
        &self,
        replay_class_hash: &ReplayClassHash,
    ) -> Result<ContractClass, DatabaseError>;

    /// Returns the header of a block from the database.
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

    /// Replays the list of transactions in a block.
    ///
    /// # Arguments
    ///
    /// - `work`: The block to be re-executed
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if any transaction fails execution or if there is any
    /// error communicating with the Pathfinder database.
    fn execute_block(&self, work: &ReplayBlock) -> Result<VisitedPcs, RunnerError>;
}
