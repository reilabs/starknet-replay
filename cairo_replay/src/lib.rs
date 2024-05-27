//! Replays transactions from the `pathfinder` sqlite database and
//! collects statistics on the execution of those transactions.
//!
//! At the current time, the library focuses on gathering usage
//! statistics of the various library functions (libfuncs) in the
//! blocks being replayed. In the future it may be expanded to
//! collect more kinds of data during replay.
//!
//! The simplest interaction with this library is to call the function
//! [`run_replay`] which returns the usage statistics of libfuncs.
//!
//! The key structs of the library are as follows:
//!
//! - [`ReplayBlock`] struct which contains a single block of transactions.
//! - [`runner::SierraCasmRunnerLight`] struct to extract profiling data from a
//!   list of visited program counters.
//! - [`DebugReplacer`] struct replaces the ids of libfuncs and types in a
//!   Sierra program.
//!
//! Beyond [`run_replay`], the other key public functions of the library are as
//! follows:
//!
//! - [`runner::extract_libfuncs_weight`] which updates the cumulative usage of
//!   libfuncs
//! - [`runner::replace_sierra_ids_in_program`] which replaces the ids of
//!   libfuncs and types with their debug name in a Sierra program.

#![warn(clippy::all, clippy::cargo, clippy::pedantic)]
#![allow(clippy::multiple_crate_versions)] // Due to duplicate dependencies in pathfinder

use std::sync::mpsc::channel;

use anyhow::Context;
use cairo_lang_utils::ordered_hash_map::OrderedHashMap;
use error::RunnerError;
use pathfinder_common::consts::{
    GOERLI_INTEGRATION_GENESIS_HASH,
    GOERLI_TESTNET_GENESIS_HASH,
    MAINNET_GENESIS_HASH,
    SEPOLIA_INTEGRATION_GENESIS_HASH,
    SEPOLIA_TESTNET_GENESIS_HASH,
};
use pathfinder_common::{BlockNumber, ChainId};
use pathfinder_executor::ExecutionState;
use pathfinder_rpc::compose_executor_transaction;
use pathfinder_storage::{
    BlockId,
    Storage,
    Transaction as DatabaseTransaction,
};
use rayon::iter::{ParallelBridge, ParallelIterator};
use runner::replay_block::ReplayBlock;
use smol_str::SmolStr;

pub use crate::pathfinder_db::{connect_to_database, get_latest_block_number};
use crate::runner::analysis::extract_libfuncs_weight;
pub use crate::runner::replay_range::ReplayRange;

mod error;
mod pathfinder_db;
mod runner;

/// Replays all transactions from `start_block` to `end_block` and gathers
/// statistics while doing so.
///
/// This function does not check that the `start_block` and `end_block` are
/// within the database history. It is expected that the user does this of their
/// own accord.
///
/// # Arguments
///
/// - `replay_range`: The range of blocks to be replayed.
/// - `storage`: Connection with the Pathfinder database
///
/// # Errors
///
/// Returns [`Err`] if:
///
/// - A block number doesn't exist in the database history
/// - `end_block` is less than `start_block`
pub fn run_replay(
    replay_range: &ReplayRange,
    storage: Storage,
) -> Result<OrderedHashMap<SmolStr, usize>, RunnerError> {
    // List of blocks to be replayed
    let replay_work: Vec<ReplayBlock> =
        generate_replay_work(replay_range, &storage)?;

    // Iterate through each block in `replay_work` and replay all the
    // transactions
    replay_blocks(storage, &replay_work)
}

/// Generates the list of transactions to be replayed.
///
/// This function queries the Pathfinder database to get the list of
/// transactions that need to be replayed. The list of transactions is taken
/// from all the transactions from `start_block` to `end_block` (inclusive).
///
/// # Arguments
///
/// - `replay_range`: The range of blocks to be replayed.
/// - `storage`: Connection with the Pathfinder database
///
/// # Errors
///
/// Returns [`Err`] if there is an issue accessing the Pathfinder database.
fn generate_replay_work(
    replay_range: &ReplayRange,
    storage: &Storage,
) -> Result<Vec<ReplayBlock>, RunnerError> {
    let mut db = storage
        .connection()
        .context("Opening sqlite database connection")?;
    let transaction = db.transaction()?;

    let start_block = replay_range.get_start_block();
    let end_block = replay_range.get_end_block();

    (start_block..=end_block)
        .map(|block_number| {
            let block_id =
                BlockId::Number(BlockNumber::new_or_panic(block_number));
            let Some(header) = transaction.block_header(block_id)? else {
                return Err(RunnerError::Unknown(
                    format!("Missing block: {block_number}",).to_string(),
                ));
            };
            let transactions_and_receipts = transaction
                .transaction_data_for_block(block_id)
                .context("Reading transactions from sqlite database")?
                .context(format!(
                    "Transaction data missing from sqlite database for block \
                     {block_number}"
                ))?;

            let (mut transactions, mut receipts): (Vec<_>, Vec<_>) =
                transactions_and_receipts.into_iter().unzip();

            transactions.truncate(1);
            receipts.truncate(1);

            ReplayBlock::new(header, transactions, receipts)
        })
        .collect::<Result<Vec<ReplayBlock>, RunnerError>>()
}

/// Re-execute the list of transactions in `replay_work` and return the
/// statistics on libfunc usage.
///
/// `replay_work` contains the lists of transactions to replay grouped by block.
///
/// # Arguments
///
/// - `replay_work`: The list of blocks to be replayed. Each index in
///   corresponds to a block.
/// - `storage`: Connection with the Pathfinder database.
///
/// # Errors
///
/// Returns [`Err`] if the function `execute_block` fails to replay any
/// transaction.
fn replay_blocks(
    storage: Storage,
    replay_work: &[ReplayBlock],
) -> Result<OrderedHashMap<SmolStr, usize>, RunnerError> {
    let (sender, receiver) = channel();
    replay_work.iter().par_bridge().try_for_each_with(
        (storage, sender),
        |(storage, sender), block| -> anyhow::Result<()> {
            let block_libfuncs_weight = execute_block(storage, block)?;
            sender.send(block_libfuncs_weight)?;
            Ok(())
        },
    )?;

    let res: Vec<_> = receiver.iter().collect();

    let mut cumulative_libfunc_stat = OrderedHashMap::default();

    for block_libfuncs in res {
        for (libfunc, weight) in block_libfuncs.iter() {
            cumulative_libfunc_stat
                .entry(libfunc.clone())
                .and_modify(|e| *e += *weight)
                .or_insert(*weight);
        }
    }
    Ok(cumulative_libfunc_stat)
}

/// Replay the list of transactions in a block.
///
/// # Arguments
///
/// - `storage`: Connection with the Pathfinder database.
/// - `work`: The block to be re-executed
///
/// # Errors
///
/// Returns [`Err`] if any transaction fails execution or if there is any error
/// communicating with the Pathfinder database.
fn execute_block(
    storage: &mut Storage,
    work: &ReplayBlock,
) -> Result<OrderedHashMap<SmolStr, usize>, RunnerError> {
    let mut db = storage.connection()?;

    let db_tx = db
        .transaction()
        .expect("Create transaction with sqlite database");

    let chain_id = get_chain_id(&db_tx)?;

    let execution_state =
        ExecutionState::trace(&db_tx, chain_id, work.header.clone(), None);

    let mut transactions = Vec::new();
    for transaction in &work.transactions {
        let transaction = compose_executor_transaction(transaction, &db_tx)?;
        transactions.push(transaction);
    }

    let skip_validate = false;
    let skip_fee_charge = false;
    let simulations = pathfinder_executor::simulate(
        execution_state,
        transactions,
        skip_validate,
        skip_fee_charge,
    ).map_err(|error| tracing::error!(block_number=%work.header.number, ?error, "Transaction re-execution failed")).unwrap();

    // Using `SmolStr` because it's coming from `LibfuncWeights`
    let mut cumulative_libfuncs_weight: OrderedHashMap<SmolStr, usize> =
        OrderedHashMap::default();
    for simulation in &simulations {
        let libfunc_transaction = extract_libfuncs_weight(
            &simulation.trace,
            work.header.number,
            &db_tx,
        )?;
        for (libfunc, weight) in libfunc_transaction.iter() {
            cumulative_libfuncs_weight
                .entry(libfunc.clone())
                .and_modify(|e| *e += *weight)
                .or_insert(*weight);
        }
    }
    Ok(cumulative_libfuncs_weight)
}

/// Get the `chain_id` of the Pathfinder database.
///
/// Detect the chain used by quering the hash of the first block in the
/// database. It can detect only Mainnet, Goerli, and Sepolia.
///
/// # Arguments
///
/// - `tx`: This is the open `Transaction` object with the databse.
///
/// # Errors
///
/// Returns [`Err`] if:
///
/// - The first block doesn't have a hash matching one of
/// the known hashes
/// - There is an error querying the database.
// TODO: Should it return to `DatabaseError`?
fn get_chain_id(tx: &DatabaseTransaction<'_>) -> Result<ChainId, RunnerError> {
    let (_, genesis_hash) = tx
        .block_id(BlockNumber::GENESIS.into())?
        .context("Getting genesis hash")?;

    let chain = match genesis_hash {
        MAINNET_GENESIS_HASH => ChainId::MAINNET,
        GOERLI_TESTNET_GENESIS_HASH => ChainId::GOERLI_TESTNET,
        GOERLI_INTEGRATION_GENESIS_HASH => ChainId::GOERLI_INTEGRATION,
        SEPOLIA_TESTNET_GENESIS_HASH => ChainId::SEPOLIA_TESTNET,
        SEPOLIA_INTEGRATION_GENESIS_HASH => ChainId::SEPOLIA_INTEGRATION,
        _ => return Err(RunnerError::Unknown("Unknown chain".to_string())),
    };

    Ok(chain)
}
