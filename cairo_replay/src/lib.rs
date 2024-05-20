#![warn(clippy::all, clippy::cargo, clippy::pedantic)]
#![allow(clippy::multiple_crate_versions)]

//! Replays transactions from `pathfinder` sqlite database
//! and prints the histogram of the usage of `libfuncs`
//! in the blocks replayed. This is the back end of the package.
//! The module runner contains the code for the profiler which counts
//! the number of `libfuncs` called during execution of the transaction.
//! It also contains the code to replace the ids of the libfuncs with their
//! respective name.

use anyhow::{bail, Context};
use cairo_lang_utils::ordered_hash_map::OrderedHashMap;
use pathfinder_common::consts::{
    GOERLI_INTEGRATION_GENESIS_HASH,
    GOERLI_TESTNET_GENESIS_HASH,
    MAINNET_GENESIS_HASH,
    SEPOLIA_INTEGRATION_GENESIS_HASH,
    SEPOLIA_TESTNET_GENESIS_HASH,
};
use pathfinder_common::receipt::Receipt;
use pathfinder_common::transaction::Transaction;
use pathfinder_common::{BlockHeader, BlockNumber, ChainId};
use pathfinder_executor::ExecutionState;
use pathfinder_storage::{BlockId, Storage};
use rayon::iter::{ParallelBridge, ParallelIterator};
use smol_str::SmolStr;

use crate::runner::analysis::analyse_tx;

mod runner;

/// `ReplayWork` contains the data to replay a single block from Starknet
/// blockchain.
#[derive(Debug, Clone, Eq, PartialEq, Default)]
struct ReplayWork {
    /// The header of the block being replayed.
    pub header: BlockHeader,
    /// The list of transactions to be replayed.
    pub transactions: Vec<Transaction>,
    /// The list of receipts after a transaction is replayed using
    /// `pathfinder` node.
    pub receipts: Vec<Receipt>,
    /// The key corresponds to the concrete libfunc name and the value
    /// contains the number of times the libfunc has been called
    /// during execution of all the transactions in the block
    pub libfuncs_weight: OrderedHashMap<SmolStr, usize>,
}

impl ReplayWork {
    pub fn new(
        header: BlockHeader,
        transactions: Vec<Transaction>,
        receipts: Vec<Receipt>,
    ) -> ReplayWork {
        Self {
            header,
            transactions,
            receipts,
            libfuncs_weight: OrderedHashMap::default(),
        }
    }

    /// Updates `self.libfuncs_weight` with the data from `libfuncs_weight`.
    /// For keys already present in `self.libfuncs_weight`, the value (i.e.
    /// weight) is added on top.
    pub fn add_libfuncs(
        &mut self,
        libfuncs_weight: &OrderedHashMap<SmolStr, usize>,
    ) {
        for (libfunc, weight) in libfuncs_weight.iter() {
            self.libfuncs_weight
                .entry(libfunc.clone())
                .and_modify(|e| *e += *weight)
                .or_insert(*weight);
        }
    }

    /// The reverse of `self.add_libfuncs`. `libfuncs_weight` is updated with
    /// data from `self.libfuncs_weight`.
    pub fn extend_libfunc_stats(
        &self,
        libfuncs_weight: &mut OrderedHashMap<SmolStr, usize>,
    ) {
        for (libfunc, weight) in self.libfuncs_weight.iter() {
            libfuncs_weight
                .entry(libfunc.clone())
                .and_modify(|e| *e += *weight)
                .or_insert(*weight);
        }
    }
}

/// `run_replay` is the entry function in this library. It replays all
/// transactions from `start_block` to `end_block`. Not checking
/// that `start_block` and `end_block` are within the limits of the database
/// history. `storage` is the connection to the `pathfinder` database.
///
/// # Errors
///
/// Returns an error if there is any error calling `generate_replay_work`
/// or if `execute` returns an error.
pub fn run_replay(
    start_block: u64,
    end_block: u64,
    storage: Storage,
) -> anyhow::Result<OrderedHashMap<SmolStr, usize>> {
    // List of blocks to be replayed
    let mut replay_work: Vec<ReplayWork> =
        generate_replay_work(start_block, end_block, &storage)?;

    // Iterate through each block in `replay_work` and replay all the
    // transactions
    replay_transactions(storage, &mut replay_work)
}

/// Query the pathfinder database to get the list of transactions that need to
/// be replayed. The list of transactions is taken from all the transactions
/// from `start_block` to `end_block`. `storage` is the connection to the
/// `pathfinder` database.
///
/// # Errors
///
/// Returns an error if there is any issue accessing the pathfinder database
fn generate_replay_work(
    start_block: u64,
    end_block: u64,
    storage: &Storage,
) -> anyhow::Result<Vec<ReplayWork>> {
    let mut db = storage
        .connection()
        .context("Opening sqlite database connection")?;
    let transaction = db.transaction()?;

    (start_block..=end_block)
        .map(|block_number| {
            let block_id =
                BlockId::Number(BlockNumber::new_or_panic(block_number));
            let Some(header) = transaction.block_header(block_id)? else {
                bail!("Missing block: {}", block_number);
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

            Ok(ReplayWork::new(header, transactions, receipts))
        })
        .collect::<anyhow::Result<Vec<ReplayWork>>>()
}

/// Re-execute the list of transactions in `replay_work`. `storage` is the
/// connection to the `pathfinder` database. `replay_work` contains the lists of
/// transactions to replay grouped by block. Each index in `replay_work`
/// corresponds to a block.
///
/// # Errors
///
/// It returns an error if the function `execute` fails execution.
fn replay_transactions(
    storage: Storage,
    replay_work: &mut Vec<ReplayWork>,
) -> anyhow::Result<OrderedHashMap<SmolStr, usize>> {
    replay_work.iter_mut().par_bridge().try_for_each_with(
        storage,
        |storage, block| -> anyhow::Result<()> {
            execute(storage, block)?;
            Ok(())
        },
    )?;

    let mut cumulative_libfunc_stat = OrderedHashMap::default();
    for block in replay_work {
        block.extend_libfunc_stats(&mut cumulative_libfunc_stat);
    }
    Ok(cumulative_libfunc_stat)
}

/// Replay the list of transactions given in the argument `work`. `storage`
/// contains the connection to the `pathfinder` database.
///
/// # Errors
///
/// It returns an error if any transaction fails execution or if there is
/// any error communicating with the sqlite database.
fn execute(storage: &mut Storage, work: &mut ReplayWork) -> anyhow::Result<()> {
    let mut db = storage.connection()?;

    let db_tx = db
        .transaction()
        .expect("Create transaction with sqlite database");

    let chain_id = get_chain_id(&db_tx)?;

    let execution_state =
        ExecutionState::trace(&db_tx, chain_id, work.header.clone(), None);

    let mut transactions = Vec::new();
    for transaction in &work.transactions {
        let transaction =
            pathfinder_rpc::compose_executor_transaction(transaction, &db_tx)?;
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
        analyse_tx(
            &simulation.trace,
            work.header.number,
            &db_tx,
            &mut cumulative_libfuncs_weight,
        );
    }
    work.add_libfuncs(&cumulative_libfuncs_weight);
    Ok(())
}

/// Detect the chain used by quering the hash of the first block in the
/// database. `tx` is the open `Transaction` object with the databse. It can
/// detect only Mainnet, Goerli, Sepolia networks.
///
/// # Errors
///
/// It returns an error if the first block doesn't have a hash matching one of
/// the known hashes or there is an error querying the database.
fn get_chain_id(
    tx: &pathfinder_storage::Transaction<'_>,
) -> anyhow::Result<ChainId> {
    let (_, genesis_hash) = tx
        .block_id(BlockNumber::GENESIS.into())?
        .context("Getting genesis hash")?;

    let chain = match genesis_hash {
        MAINNET_GENESIS_HASH => ChainId::MAINNET,
        GOERLI_TESTNET_GENESIS_HASH => ChainId::GOERLI_TESTNET,
        GOERLI_INTEGRATION_GENESIS_HASH => ChainId::GOERLI_INTEGRATION,
        SEPOLIA_TESTNET_GENESIS_HASH => ChainId::SEPOLIA_TESTNET,
        SEPOLIA_INTEGRATION_GENESIS_HASH => ChainId::SEPOLIA_INTEGRATION,
        _ => anyhow::bail!("Unknown chain"),
    };

    Ok(chain)
}
