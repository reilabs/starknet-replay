//! This module is an interface between the Pathfinder database API and
//! `starknet-replay`.

use std::num::NonZeroU32;
use std::path::PathBuf;

use anyhow::Context;
use pathfinder_common::consts::{
    GOERLI_INTEGRATION_GENESIS_HASH,
    GOERLI_TESTNET_GENESIS_HASH,
    MAINNET_GENESIS_HASH,
    SEPOLIA_INTEGRATION_GENESIS_HASH,
    SEPOLIA_TESTNET_GENESIS_HASH,
};
use pathfinder_common::receipt::Receipt;
use pathfinder_common::transaction::Transaction as StarknetTransaction;
use pathfinder_common::{BlockHeader, BlockNumber as PathfinderBlockNumber, ChainId, ClassHash};
use pathfinder_executor::IntoFelt;
use pathfinder_rpc::v02::types::ContractClass;
use pathfinder_storage::{BlockId, JournalMode, Storage};
use rayon::current_num_threads;
use starknet_api::hash::StarkFelt;

use crate::common::BlockNumber;
use crate::error::DatabaseError;
use crate::runner::replay_class_hash::ReplayClassHash;

/// Connects to the Pathfinder database
///
/// The connection to the Pathfinder database is established with the
/// construction of the Storage object.
///
/// The number of parallel connections is set to be twice the number of threads
/// in the CPU: this is to ensure spare capacity. In case of panics, the default
/// number of connections is set to 1.
///
/// # Arguments
///
/// - `database_path`: Path of the Pathfinder database file.
///
/// # Errors
///
/// Returns [`Err`] if this function is called more than once in the
/// application.
pub fn connect_to_database(database_path: PathBuf) -> Result<Storage, DatabaseError> {
    let n_cpus = current_num_threads();
    let n_parallel_connections: u32 = n_cpus.checked_mul(2).unwrap_or(1).try_into().unwrap_or(1);
    let Some(capacity) = NonZeroU32::new(n_parallel_connections) else {
        unreachable!("n_parallel_connections should never be less than 1.")
    };

    tracing::info!("Pathfinder db capacity {capacity}");

    let store_manager = Storage::migrate(database_path, JournalMode::WAL, 1)
        .map_err(DatabaseError::ConnectToDatabase)?;
    let pool = store_manager
        .create_pool(capacity)
        .map_err(DatabaseError::ConnectToDatabase)?;
    Ok(pool)
}

/// Returns the latest (most recent) block number in the database
///
/// If no block is found in the database, it returns 0.
///
/// # Arguments
///
/// - `storage`: The `Storage` object of the Pathfinder database connection.
///
/// # Errors
///
/// Returns [`Err`] if the low level API with the database returns an error.
pub fn get_latest_block_number(storage: &Storage) -> Result<u64, DatabaseError> {
    let mut db = storage
        .connection()
        .context("Opening database connection")
        .map_err(DatabaseError::GetLatestBlockNumber)?;
    let tx_db = db
        .transaction()
        .map_err(DatabaseError::GetLatestBlockNumber)?;

    let Some((latest_block, _)) = tx_db
        .block_id(BlockId::Latest)
        .map_err(DatabaseError::GetLatestBlockNumber)?
    else {
        return Ok(0);
    };
    Ok(latest_block.get())
}

/// Get the `chain_id` of the Pathfinder database.
///
/// This function detects the chain used by quering the hash of the first block
/// in the database. It can detect only Mainnet, Goerli, and Sepolia.
///
/// # Arguments
///
/// - `storage`: The `Storage` object of the Pathfinder database connection.
///
/// # Errors
///
/// Returns [`Err`] if:
///
/// - The first block doesn't have a hash matching one of
/// the known hashes
/// - There is an error querying the database.
pub fn get_chain_id(storage: &Storage) -> Result<ChainId, DatabaseError> {
    let mut db = storage
        .connection()
        .context("Opening database connection")
        .map_err(DatabaseError::GetChainId)?;
    let tx_db = db.transaction().map_err(DatabaseError::GetChainId)?;

    let (_, genesis_hash) = tx_db
        .block_id(PathfinderBlockNumber::GENESIS.into())
        .map_err(DatabaseError::GetChainId)?
        .context("Getting genesis hash")
        .map_err(DatabaseError::GetChainId)?;

    let chain = match genesis_hash {
        MAINNET_GENESIS_HASH => ChainId::MAINNET,
        GOERLI_TESTNET_GENESIS_HASH => ChainId::GOERLI_TESTNET,
        GOERLI_INTEGRATION_GENESIS_HASH => ChainId::GOERLI_INTEGRATION,
        SEPOLIA_TESTNET_GENESIS_HASH => ChainId::SEPOLIA_TESTNET,
        SEPOLIA_INTEGRATION_GENESIS_HASH => ChainId::SEPOLIA_INTEGRATION,
        _ => return Err(DatabaseError::Unknown("Unknown chain".to_string())),
    };

    Ok(chain)
}

/// Returns the `ContractClass` object of a `class_hash` at `block_num` from the
/// Pathfinder database `db`.
///
/// # Arguments
///
/// - `replay_class_hash`: The class hash of the `ContractClass` to return.
/// - `storage`: The `Storage` object of the Pathfinder database connection.
///
/// # Errors
///
/// Returns [`Err`] if `class_hash` doesn't exist at block `block_num` in `db`.
pub fn get_contract_class_at_block(
    replay_class_hash: &ReplayClassHash,
    storage: &Storage,
) -> Result<ContractClass, DatabaseError> {
    let mut db = storage
        .connection()
        .context("Opening database connection")
        .map_err(DatabaseError::GetContractClassAtBlock)?;
    let tx_db = db
        .transaction()
        .map_err(DatabaseError::GetContractClassAtBlock)?;

    let block_number = replay_class_hash.block_number;
    let block_id = BlockId::Number(block_number);

    let class_hash: StarkFelt = replay_class_hash.class_hash.0;
    let class_definition = tx_db.class_definition_at(block_id, ClassHash(class_hash.into_felt()));
    let class_definition = class_definition
        .map_err(DatabaseError::GetContractClassAtBlock)?
        .ok_or(DatabaseError::ContractClassNotFound {
            block_id,
            class_hash,
        })?;

    let contract_class = ContractClass::from_definition_bytes(&class_definition)
        .map_err(DatabaseError::GetContractClassAtBlock)?;
    Ok(contract_class)
}

/// Returns the header of a block from the database.
///
/// # Arguments
///
/// - `block_number`: The block to query.
/// - `storage`: The `Storage` object of the Pathfinder database connection.
///
/// # Errors
///
/// Returns [`Err`] if `block_id` doesn't exist.
pub fn get_block_header(
    block_number: BlockNumber,
    storage: &Storage,
) -> Result<BlockHeader, DatabaseError> {
    let mut db = storage
        .connection()
        .context("Opening database connection")
        .map_err(DatabaseError::GetBlockHeader)?;
    let tx_db = db.transaction().map_err(DatabaseError::GetBlockHeader)?;

    let block_id = BlockId::Number(block_number.into());

    let Some(header) = tx_db
        .block_header(block_id)
        .map_err(DatabaseError::GetBlockHeader)?
    else {
        return Err(DatabaseError::BlockNotFound { block_id });
    };
    Ok(header)
}

/// Returns the transactions and transaction receipts of a block.
///
/// # Arguments
///
/// - `block_number`: The block to query.
/// - `storage`: The `Storage` object of the Pathfinder database connection.
///
/// # Errors
///
/// Returns [`Err`] if `block_id` doesn't exist or there are no transactions.
pub fn get_transactions_and_receipts_for_block(
    block_number: BlockNumber,
    storage: &Storage,
) -> Result<(Vec<StarknetTransaction>, Vec<Receipt>), DatabaseError> {
    let mut db = storage
        .connection()
        .context("Opening database connection")
        .map_err(DatabaseError::GetTransactionsAndReceipts)?;
    let tx_db = db
        .transaction()
        .map_err(DatabaseError::GetTransactionsAndReceipts)?;

    let block_id = BlockId::Number(block_number.into());

    let transactions_and_receipts = tx_db
        .transaction_data_for_block(block_id)
        .map_err(DatabaseError::GetTransactionsAndReceipts)?;
    let transactions_and_receipts: Vec<(StarknetTransaction, Receipt)> = transactions_and_receipts
        .ok_or(DatabaseError::GetTransactionsAndReceiptsNotFound { block_id })?;

    let (transactions, receipts): (Vec<_>, Vec<_>) = transactions_and_receipts.into_iter().unzip();
    Ok((transactions, receipts))
}
