//! This module is an interface between the Pathfinder database API and
//! `starknet-replay`.

// Allowing `module_name_repetitions` is needed to make `clippy` happy and keep the suffix `Error`
// for all the error categories. Alternatively, shortening the name would limit expressiveness of
// the type in this case.
#![allow(clippy::module_name_repetitions)]

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
use pathfinder_executor::types::TransactionSimulation;
use pathfinder_executor::{ExecutionState, IntoFelt};
use pathfinder_rpc::compose_executor_transaction;
use pathfinder_rpc::v02::types::ContractClass;
use pathfinder_storage::{BlockId, JournalMode, Storage};
use rayon::current_num_threads;
use starknet_api::hash::StarkFelt;

use crate::block_number::BlockNumber;
use crate::error::{DatabaseError, RunnerError};
use crate::runner::replay_block::ReplayBlock;
use crate::runner::replay_class_hash::ReplayClassHash;
use crate::storage::Storage as ReplayStorage;

/// Implements the trait [`crate::storage::Storage`] to interact with
/// the Pathfinder storage layer.
#[derive(Clone)]
pub struct PathfinderStorage {
    /// The [`pathfinder_storage::Storage`] object to create a connection with
    /// the `Pathfinder` database.
    storage: Storage,
}
impl PathfinderStorage {
    /// Connects to the Pathfinder database.
    ///
    /// The connection to the Pathfinder database is established with the
    /// construction of the [`pathfinder_storage::Storage`] object.
    ///
    /// The number of parallel connections is set to be twice the number of
    /// threads in the CPU: this is to ensure spare capacity. In case of
    /// panics, the default number of connections is set to 1.
    ///
    /// # Arguments
    ///
    /// - `database_path`: Path of the Pathfinder database file.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if this function is called more than once in the
    /// application.
    pub fn new(database_path: PathBuf) -> Result<Self, DatabaseError> {
        let n_cpus = current_num_threads();
        let n_parallel_connections: u32 =
            n_cpus.checked_mul(2).unwrap_or(1).try_into().unwrap_or(1);
        let Some(capacity) = NonZeroU32::new(n_parallel_connections) else {
            unreachable!("n_parallel_connections should never be less than 1.")
        };

        tracing::info!("Pathfinder db capacity {capacity}");

        let store_manager = Storage::migrate(database_path, JournalMode::WAL, 1)
            .map_err(DatabaseError::ConnectToDatabase)?;
        let storage = store_manager
            .create_pool(capacity)
            .map_err(DatabaseError::ConnectToDatabase)?;
        Ok(PathfinderStorage { storage })
    }

    #[must_use]
    pub fn get(&self) -> &Storage {
        &self.storage
    }
}
impl ReplayStorage for PathfinderStorage {
    fn get_most_recent_block_number(&self) -> Result<BlockNumber, DatabaseError> {
        let mut db = self
            .storage
            .connection()
            .context("Opening database connection")
            .map_err(DatabaseError::GetMostRecentBlockNumber)?;
        let tx_db = db
            .transaction()
            .map_err(DatabaseError::GetMostRecentBlockNumber)?;

        let Some((latest_block, _)) = tx_db
            .block_id(BlockId::Latest)
            .map_err(DatabaseError::GetMostRecentBlockNumber)?
        else {
            return Ok(BlockNumber::new(0));
        };
        Ok(BlockNumber::new(latest_block.get()))
    }

    /// This function detects the chain id by quering the hash of the first
    /// block in the database.
    fn get_chain_id(&self) -> Result<ChainId, DatabaseError> {
        let mut db = self
            .storage
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

    fn get_contract_class_at_block(
        &self,
        replay_class_hash: &ReplayClassHash,
    ) -> Result<ContractClass, DatabaseError> {
        let mut db = self
            .storage
            .connection()
            .context("Opening database connection")
            .map_err(DatabaseError::GetContractClassAtBlock)?;
        let tx_db = db
            .transaction()
            .map_err(DatabaseError::GetContractClassAtBlock)?;

        let block_number = replay_class_hash.block_number;
        let block_id = BlockId::Number(block_number.into());

        let class_hash: StarkFelt = replay_class_hash.class_hash.0;
        let class_definition =
            tx_db.class_definition_at(block_id, ClassHash(class_hash.into_felt()));
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

    fn get_block_header(&self, block_number: BlockNumber) -> Result<BlockHeader, DatabaseError> {
        let mut db = self
            .storage
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

    fn get_transactions_and_receipts_for_block(
        &self,
        block_number: BlockNumber,
    ) -> Result<(Vec<StarknetTransaction>, Vec<Receipt>), DatabaseError> {
        let mut db = self
            .storage
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
        let transactions_and_receipts: Vec<(StarknetTransaction, Receipt)> =
            transactions_and_receipts
                .ok_or(DatabaseError::GetTransactionsAndReceiptsNotFound { block_id })?;

        let (transactions, receipts): (Vec<_>, Vec<_>) =
            transactions_and_receipts.into_iter().unzip();
        Ok((transactions, receipts))
    }

    fn execute_block(&self, work: &ReplayBlock) -> Result<Vec<TransactionSimulation>, RunnerError> {
        let chain_id = self.get_chain_id()?;

        let mut db = self.get().connection().map_err(RunnerError::ExecuteBlock)?;
        let db_tx = db
            .transaction()
            .expect("Create transaction with sqlite database");
        let execution_state = ExecutionState::trace(&db_tx, chain_id, work.header.clone(), None);

        let mut transactions = Vec::new();
        for transaction in &work.transactions {
            let transaction = compose_executor_transaction(transaction, &db_tx)
                .map_err(RunnerError::ExecuteBlock)?;
            transactions.push(transaction);
        }

        let skip_validate = false;
        let skip_fee_charge = false;
        let simulations = pathfinder_executor::simulate(
            execution_state,
            transactions,
            skip_validate,
            skip_fee_charge,
        ).map_err(|error| {
            tracing::error!(block_number=%work.header.number, ?error, "Transaction re-execution failed");
            error
        })?;

        Ok(simulations)
        // Ok(simulations
        //     .into_iter()
        //     .map(|t| {
        //         let block_number: BlockNumber = work.header.number.into();
        //         TransactionTrace::new(block_number, t.trace)
        //     })
        //     .collect())
    }
}
