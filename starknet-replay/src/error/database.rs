//! This file contains the enum `Error` for all the errors returned by the
//! module `pathfinder_db`.

use starknet_api::hash::StarkFelt;
use thiserror::Error;

use crate::block_number::BlockNumber;

#[derive(Debug, Error)]
pub enum Error {
    /// `ConnectToDatabase` is used to encapsulate errors of type
    /// [`anyhow::Error`] which are originating from the
    /// function [`crate::storage::pathfinder::PathfinderStorage::new`].
    #[error(transparent)]
    ConnectToDatabase(anyhow::Error),

    /// `GetLatestBlockNumber` is used to encapsulate errors of type
    /// [`anyhow::Error`] which are originating from the
    /// function [`crate::storage::pathfinder::PathfinderStorage#method.
    /// get_most_recent_block_number`].
    #[error(transparent)]
    GetMostRecentBlockNumber(anyhow::Error),

    /// `GetBlockHeader` is used to encapsulate errors of type
    /// [`anyhow::Error`] which are originating from the function
    /// [`crate::storage::pathfinder::PathfinderStorage#method.
    /// get_block_header`.
    #[error(transparent)]
    GetBlockHeader(anyhow::Error),

    /// `GetContractClassAtBlock` is used to encapsulate errors of type
    /// [`anyhow::Error`] which are originating from the function
    /// [`crate::storage::pathfinder::PathfinderStorage#method.
    /// get_contract_class_at_block`].
    #[error(transparent)]
    GetContractClassAtBlock(anyhow::Error),

    /// `GetTransactionsAndReceipts` is used to encapsulate errors of type
    /// [`anyhow::Error`] which are originating from the function
    /// [`crate::storage::pathfinder::PathfinderStorage#method.
    /// get_transactions_and_receipts_for_block`].
    #[error(transparent)]
    GetTransactionsAndReceipts(anyhow::Error),

    /// `GetTransactionsAndReceiptsNotFound` is used for `None` results from the
    /// database in the function
    /// [`crate::storage::pathfinder::PathfinderStorage#method.
    /// get_transactions_and_receipts_for_block`].
    #[error("Transactions for block {block_id:?} not found.")]
    GetTransactionsAndReceiptsNotFound { block_id: BlockNumber },

    /// `ContractClassNotFound` is used for `None` results from the database in
    /// the function
    /// [`crate::storage::pathfinder::PathfinderStorage#method.
    /// get_contract_class_at_block`].
    #[error("Contract Class {class_hash:?} not found in Database at block {block_id:?}.")]
    ContractClassNotFound {
        block_id: BlockNumber,
        class_hash: StarkFelt,
    },

    #[error(transparent)]
    MinReq(#[from] jsonrpc::minreq_http::Error),

    #[error(transparent)]
    JsonRpc(#[from] jsonrpc::Error),

    /// `GetChainId` is used to encapsulate errors of type [`anyhow::Error`]
    /// which are originating from the function
    /// [`crate::storage::pathfinder::PathfinderStorage#method.
    /// get_chain_id`].
    #[error(transparent)]
    GetChainId(anyhow::Error),

    /// The `Unknown` variant is for any other uncategorised error.
    #[error("Unknown Error communicating with Pathfinder database: {0:?}")]
    Unknown(String),
}
