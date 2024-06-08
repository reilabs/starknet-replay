//! This file contains the enum `Error` for all the errors returned by the
//! module `pathfinder_db`.

use pathfinder_storage::BlockId;
use starknet_api::hash::StarkFelt;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// `ConnectToDatabase` is used to encapsulate errors of type
    /// `anyhow::Error` which are originating from the
    /// function `starknet_replay::runner::pathfinder_db::connect_to_database`.
    #[error(transparent)]
    ConnectToDatabase(anyhow::Error),

    /// `BlockNotFound` variant is returned when the block requested from the
    /// Pathfinder database isn't found.
    #[error("Block number {block_id:?} not found in database.")]
    BlockNotFound { block_id: BlockId },

    /// `GetLatestBlockNumber` is used to encapsulate errors of type
    /// `anyhow::Error` which are originating from the
    /// function `starknet_replay::runner::pathfinder_db::get_latest_block_number`.
    #[error(transparent)]
    GetLatestBlockNumber(anyhow::Error),

    /// `GetBlockHeader` is used to encapsulate errors of type
    /// `anyhow::Error` which are originating from the
    /// function `starknet_replay::runner::pathfinder_db::get_block_header`.
    #[error(transparent)]
    GetBlockHeader(anyhow::Error),

    /// `GetContractClassAtBlock` is used to encapsulate errors of type
    /// `anyhow::Error` which are originating from the
    /// function `starknet_replay::runner::pathfinder_id::get_contract_class_at_block`.
    #[error(transparent)]
    GetContractClassAtBlock(anyhow::Error),

    /// `GetTransactionsAndReceipts` is used to encapsulate errors of type
    /// `anyhow::Error` which are originating from the
    /// function `starknet_replay::runner::pathfinder_id::get_transactions_and_receipts_for_block`.
    #[error(transparent)]
    GetTransactionsAndReceipts(anyhow::Error),

    /// `GetTransactionsAndReceiptsNotFound` is used for `None` results from the
    /// database in the function
    /// `starknet_replay::runner::pathfinder_id::get_transactions_and_receipts_for_block`.
    #[error("Transactions for block {block_id:?} not found.")]
    GetTransactionsAndReceiptsNotFound { block_id: BlockId },

    /// `ContractClassNotFound` is used for `None` results from the database in
    /// the function
    /// `starknet_replay::runner::pathfinder_id::get_contract_class_at_block`.
    #[error("Contract Class {class_hash:?} not found in Database at block {block_id:?}.")]
    ContractClassNotFound {
        block_id: BlockId,
        class_hash: StarkFelt,
    },

    /// `GetChainId` is used to encapsulate errors of type
    /// `anyhow::Error` which are originating from the
    /// function `starknet_replay::runner::pathfinder_db::get_chain_id`.
    #[error(transparent)]
    GetChainId(anyhow::Error),

    /// The `Unknown` variant is for any other uncategorised error.
    #[error("Unknown Error communicating with Pathfinder database: {0:?}")]
    Unknown(String),
}
