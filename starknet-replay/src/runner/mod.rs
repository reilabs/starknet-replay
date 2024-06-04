//! The module runner contains the code to replay transactions and extract the
//! sequence of visited program counters from each transaction replayed.

use std::collections::HashMap;
use std::sync::mpsc::channel;

use anyhow::Context;
use pathfinder_common::BlockNumber;
use pathfinder_executor::types::TransactionTrace;
use pathfinder_executor::ExecutionState;
use pathfinder_rpc::compose_executor_transaction;
use pathfinder_storage::{BlockId, Storage};
use rayon::iter::{ParallelBridge, ParallelIterator};
use starknet_api::core::ClassHash as StarknetClassHash;

use self::visited_pcs::ReplayClassHash;
use crate::runner::pathfinder_db::get_chain_id;
pub use crate::runner::visited_pcs::VisitedPcs;
use crate::{ReplayBlock, ReplayRange, RunnerError};

pub mod pathfinder_db;
pub mod replay_block;
pub mod replay_range;
pub mod visited_pcs;

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
/// - `storage`: Connection with the Pathfinder database.
///
/// # Errors
///
/// Returns [`Err`] if:
///
/// - A block number doesn't exist in the database history.
/// - `end_block` is less than `start_block`.
pub fn run_replay(replay_range: &ReplayRange, storage: Storage) -> Result<VisitedPcs, RunnerError> {
    // List of blocks to be replayed
    let replay_work: Vec<ReplayBlock> = generate_replay_work(replay_range, &storage)?;

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
/// - `storage`: Connection with the Pathfinder database.
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
        .context("Opening sqlite database connection")
        .map_err(RunnerError::GenerateReplayWork)?;
    let transaction = db.transaction().map_err(RunnerError::GenerateReplayWork)?;

    let start_block = replay_range.get_start_block();
    let end_block = replay_range.get_end_block();

    (start_block..=end_block)
        .map(|block_number| {
            let block_id = BlockId::Number(BlockNumber::new_or_panic(block_number));
            let Some(header) = transaction
                .block_header(block_id)
                .map_err(RunnerError::GenerateReplayWork)?
            else {
                return Err(RunnerError::Unknown(
                    format!("Missing block: {block_number}",).to_string(),
                ));
            };
            let transactions_and_receipts = transaction
                .transaction_data_for_block(block_id)
                .context("Reading transactions from sqlite database")
                .map_err(RunnerError::GenerateReplayWork)?
                .context(format!(
                    "Transaction data missing from sqlite database for block {block_number}"
                ))
                .map_err(RunnerError::GenerateReplayWork)?;

            let (transactions, receipts): (Vec<_>, Vec<_>) =
                transactions_and_receipts.into_iter().unzip();

            let transactions_to_process = transactions.len();
            tracing::info!(
                "{transactions_to_process} transactions to process in block {block_number}"
            );

            ReplayBlock::new(header, transactions, receipts)
        })
        .collect::<Result<Vec<ReplayBlock>, RunnerError>>()
}

/// Re-executes the list of transactions in `replay_work` and return the
/// statistics on libfunc usage.
///
/// `replay_work` contains the list of transactions to replay grouped by block.
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
fn replay_blocks(storage: Storage, replay_work: &[ReplayBlock]) -> Result<VisitedPcs, RunnerError> {
    let (sender, receiver) = channel();
    replay_work
        .iter()
        .par_bridge()
        .try_for_each_with(
            (storage, sender),
            |(storage, sender), block| -> anyhow::Result<()> {
                let block_visited_pcs = execute_block(storage, block)?;
                sender.send(block_visited_pcs)?;
                Ok(())
            },
        )
        .map_err(RunnerError::ReplayBlocks)?;

    let res: Vec<_> = receiver.iter().collect();

    let mut cumulative_visited_pcs = VisitedPcs::default();

    for visited_pcs in res {
        cumulative_visited_pcs.extend(visited_pcs.iter().map(|(k, v)| (*k, v.clone())));
    }
    Ok(cumulative_visited_pcs)
}

/// Returns the hashmap of visited program counters for the input `trace`.
///
/// The result of `get_visited_program_counters` is a hashmap where the key is
/// the `StarknetClassHash` and the value is the Vector of visited program
/// counters for each `StarknetClassHash` execution in `trace`.
///
/// If `trace` is not an Invoke transaction, the function returns None because
/// no libfuncs have been called during the transaction execution.
///
/// # Arguments
///
/// - `trace`: the `TransactionTrace` to extract the visited program counters
///   from.
fn get_visited_program_counters(
    trace: &TransactionTrace,
) -> Option<&HashMap<StarknetClassHash, Vec<Vec<usize>>>> {
    match trace {
        TransactionTrace::Invoke(tx) => Some(&tx.visited_pcs),
        _ => None,
    }
}

/// Replays the list of transactions in a block.
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
fn execute_block(storage: &mut Storage, work: &ReplayBlock) -> Result<VisitedPcs, RunnerError> {
    let mut db = storage.connection().map_err(RunnerError::ExecuteBlock)?;

    let db_tx = db
        .transaction()
        .expect("Create transaction with sqlite database");

    let chain_id = get_chain_id(&db_tx)?;

    let execution_state = ExecutionState::trace(&db_tx, chain_id, work.header.clone(), None);

    let mut transactions = Vec::new();
    for transaction in &work.transactions {
        let transaction =
            compose_executor_transaction(transaction, &db_tx).map_err(RunnerError::ExecuteBlock)?;
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

    let mut cumulative_visited_pcs = VisitedPcs::default();
    for simulation in &simulations {
        let Some(visited_pcs) = get_visited_program_counters(&simulation.trace) else {
            continue;
        };
        cumulative_visited_pcs.extend(visited_pcs.iter().map(|(k, v)| {
            let replay_class_hash = ReplayClassHash {
                block_number: work.header.number,
                class_hash: *k,
            };
            let pcs = v.clone();
            (replay_class_hash, pcs)
        }));
    }
    Ok(cumulative_visited_pcs)
}
