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
use itertools::Itertools;
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

#[derive(Debug, Clone, Eq, PartialEq, Default)]
struct ReplayWork {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
    pub receipts: Vec<Receipt>,
}

/// `run_replay` is the entry function in this library. It replays all
/// transactions from `start_block` to `end_block`. Not checking
/// that `start_block` and `end_block` are within the limits of the database
/// history. `db_path` is the file of the `pathfinder` database.
/// If there are no execution errors, it returns the number of transactions
/// processed.
///
/// # Errors
///
/// Returns an error if there is any issue retrieving data from the database
/// or if `execute` returns an error.
pub fn run_replay(
    start_block: u64,
    end_block: u64,
    storage: Storage,
) -> anyhow::Result<usize> {
    let mut num_transactions = 0;
    let mut db = storage
        .connection()
        .context("Opening database connection")?;
    let transaction = db.transaction()?;
    let chain_id = get_chain_id(&transaction)?;

    let replay_work: Vec<ReplayWork> = (start_block..=end_block)
        .map(|block_number| {
            let block_id =
                BlockId::Number(BlockNumber::new_or_panic(block_number));
            let Some(block_header) = transaction.block_header(block_id)? else {
                bail!("Missing block: {}", block_number);
            };
            let transactions_and_receipts = transaction
                .transaction_data_for_block(block_id)
                .context("Reading transactions from database")?
                .context("Transaction data missing")?;

            let (mut transactions, mut receipts): (Vec<_>, Vec<_>) =
                transactions_and_receipts.into_iter().unzip();

            transactions.truncate(1);
            receipts.truncate(1);

            num_transactions += transactions.len();

            Ok(ReplayWork {
                header: block_header,
                transactions,
                receipts,
            })
        })
        .collect::<anyhow::Result<Vec<ReplayWork>>>()?;
    replay_work
        .into_iter()
        .par_bridge()
        .try_for_each_with(storage, |storage, block| {
            execute(storage, chain_id, block)
        })?;
    Ok(num_transactions)
}

fn execute(
    storage: &mut Storage,
    chain_id: ChainId,
    work: ReplayWork,
) -> anyhow::Result<()> {
    let mut db = storage.connection()?;

    let db_tx = db.transaction().expect("Create transaction");

    let execution_state =
        ExecutionState::trace(&db_tx, chain_id, work.header.clone(), None);

    let mut transactions = Vec::new();
    for transaction in work.transactions {
        let transaction =
            pathfinder_rpc::compose_executor_transaction(&transaction, &db_tx)?;
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
    println!("Weight by concrete libfunc:");
    for (concrete_name, weight) in cumulative_libfuncs_weight
        .iter()
        .sorted_by(|a, b| Ord::cmp(&a.1, &b.1))
    {
        println!("  libfunc {concrete_name}: {weight}");
    }
    Ok(())
}

// Detect the chain from the hash of the first block in the database
fn get_chain_id(
    tx: &pathfinder_storage::Transaction<'_>,
) -> anyhow::Result<ChainId> {
    let (_, genesis_hash) = tx
        .block_id(BlockNumber::GENESIS.into())
        .unwrap()
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
