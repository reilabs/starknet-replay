//! The module runner contains the code to replay transactions and extract the
//! sequence of visited program counters from each transaction replayed.

use std::path::PathBuf;
use std::sync::mpsc::channel;

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use self::replay_class_hash::TransactionOutput;
use self::report::write_to_file;
use crate::block_number::BlockNumber;
use crate::runner::replay_class_hash::VisitedPcs;
use crate::runner::replay_range::ReplayRange;
use crate::storage::Storage;
use crate::{ReplayBlock, RunnerError};

pub mod replay_block;
pub mod replay_class_hash;
pub mod replay_range;
pub mod replay_state_reader;
pub mod report;

/// Replays transactions as indicated by `replay_range` and extracts the list of
/// visited program counters.
///
/// # Arguments
///
/// - `replay_range`: The range of blocks to be replayed.
/// - `trace_out`: The location to save the output trace of the replayed
///   transactions.
/// - `storage`: The object to query the starknet blockchain using the RPC
///   protocol.
///
/// # Errors
///
/// Returns [`Err`] if:
///
/// - The most recent block available in the database is less than the block to
///   start the replay.
/// - There is any error during transaction replay.
pub fn run_replay<T>(
    replay_range: &ReplayRange,
    trace_out: &Option<PathBuf>,
    storage: &T,
) -> Result<VisitedPcs, RunnerError>
where
    T: Storage + Sync + Send,
{
    // List of blocks to be replayed
    let replay_work: Vec<ReplayBlock> = generate_replay_work(replay_range, storage)?;

    // Iterate through each block in `replay_work` and replay all the
    // transactions
    replay_blocks(storage, trace_out, &replay_work)
}

/// Generates the list of transactions to be replayed.
///
/// This function queries the Starknet blockchain to get the list of
/// transactions that need to be replayed.
///
/// # Arguments
///
/// - `replay_range`: The range of blocks to be replayed.
/// - `storage`: the object to query the starknet blockchain using the RPC
///   protocol.
///
/// # Errors
///
/// Returns [`Err`] if:
///
/// - There is an issue querying Starknet data.
/// - The most recent block available in the database is less than the block to
///   start the replay.
pub fn generate_replay_work<T>(
    replay_range: &ReplayRange,
    storage: &T,
) -> Result<Vec<ReplayBlock>, RunnerError>
where
    T: Storage + ?Sized,
{
    let start_block = replay_range.get_start_block();
    let end_block = replay_range.get_end_block();

    let latest_block = storage.get_most_recent_block_number()?;

    let last_block = end_block.min(latest_block);

    if start_block > last_block {
        return Err(RunnerError::InsufficientBlocks {
            last_block,
            start_block,
        });
    }

    let number_of_blocks = (last_block.get() - start_block.get() + 1).try_into()?;
    let mut replay_blocks: Vec<ReplayBlock> = Vec::with_capacity(number_of_blocks);

    for block_number in start_block.get()..=last_block.get() {
        let block_number = BlockNumber::new(block_number);

        let (transactions, receipts) =
            storage.get_transactions_and_receipts_for_block(block_number)?;

        let transactions_to_process = transactions.len();
        tracing::info!(
            "{transactions_to_process} transactions to process in block {block_number:?}"
        );

        let header = storage.get_block_header(block_number)?;
        let replay_block = ReplayBlock::new(header, transactions, receipts)?;
        replay_blocks.push(replay_block);
    }

    Ok(replay_blocks)
}

/// Generated the [`VisitedPcs`] from the list of transaction traces.
///
/// # Arguments
///
/// - `transaction_simulations`: The list of transaction traces from the
///   replayer.
#[must_use]
pub fn process_transaction_traces(transaction_simulations: Vec<TransactionOutput>) -> VisitedPcs {
    let mut cumulative_visited_pcs = VisitedPcs::default();
    for simulation in transaction_simulations {
        let visited_pcs = simulation.1;
        if visited_pcs.is_empty() {
            continue;
        }

        for (contract, pcs) in visited_pcs {
            let key = cumulative_visited_pcs.entry(contract).or_insert(Vec::new());
            key.extend(pcs.into_iter());
        }
    }
    cumulative_visited_pcs
}

/// Re-executes the list of transactions in `replay_work` and return the
/// statistics on libfunc usage.
///
/// `replay_work` contains the list of transactions to replay grouped by block.
///
/// # Arguments
///
/// - `storage`: The object to query the starknet blockchain using the RPC
///   protocol.
/// - `trace_out`: The output file of the transaction traces.
/// - `replay_work`: The list of blocks to be replayed.
///
/// # Errors
///
/// Returns [`Err`] if the function `execute_block` fails to replay any
/// transaction.
pub fn replay_blocks<T>(
    storage: &T,
    trace_out: &Option<PathBuf>,
    replay_work: &[ReplayBlock],
) -> Result<VisitedPcs, RunnerError>
where
    T: Storage + Sync + Send,
{
    let (sender, receiver) = channel();
    replay_work
        .par_iter()
        .try_for_each_with(
            (storage, trace_out, sender),
            |(storage, trace_out, sender), block| -> anyhow::Result<()> {
                let block_transaction_traces = storage.execute_block(block)?;
                let block_number = BlockNumber::new(block.header.block_number.0);
                tracing::info!("Simulation completed block {block_number}");
                if let Some(filename) = trace_out {
                    write_to_file(filename, &block_transaction_traces)?;
                }
                tracing::info!("Saved transaction trace block {block_number}");
                let visited_pcs = process_transaction_traces(block_transaction_traces);
                sender.send(visited_pcs)?;
                Ok(())
            },
        )
        .map_err(RunnerError::ReplayBlocks)?;

    let res: Vec<_> = receiver.iter().collect();

    let mut cumulative_visited_pcs = VisitedPcs::default();
    for visited_pcs in res {
        cumulative_visited_pcs.extend(visited_pcs.into_iter());
    }

    Ok(cumulative_visited_pcs)
}
