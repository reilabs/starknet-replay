use std::num::NonZeroU32;
use std::path::PathBuf;

use anyhow::bail;
use anyhow::Context;
use cairo_lang_utils::ordered_hash_map::OrderedHashMap;
use itertools::Itertools;
use pathfinder_common::receipt::Receipt;
use pathfinder_common::transaction::Transaction;
use pathfinder_common::{BlockHeader, BlockNumber, ChainId};
use pathfinder_executor::ExecutionState;
use pathfinder_storage::{BlockId, JournalMode, Storage};
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

pub fn run_replay(
    start_block: u64,
    end_block: u64,
    db_path: PathBuf,
    chain_id: ChainId,
) -> anyhow::Result<usize> {
    let mut num_transactions = 0;

    let n_cpus = rayon::current_num_threads();
    let storage = Storage::migrate(db_path, JournalMode::WAL, 1)?
        .create_pool(NonZeroU32::new(n_cpus as u32 * 2).unwrap())?;
    let mut db = storage
        .connection()
        .context("Opening database connection")?;

    let replay_work: Vec<ReplayWork> = (start_block..=end_block)
        .map(|block_number| {
            let transaction = db.transaction()?;
            let block_id = BlockId::Number(BlockNumber::new_or_panic(block_number));
            let Some(block_header) = transaction.block_header(block_id)? else {
                bail!("Missing block: {}", block_number);
            };
            let transactions_and_receipts = transaction
                .transaction_data_for_block(block_id)
                .context("Reading transactions from database")?
                .context("Transaction data missing")?;
            drop(transaction);

            let (transactions, receipts): (Vec<_>, Vec<_>) =
                transactions_and_receipts.into_iter().unzip();

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
        .for_each_with(storage, |storage, block| {
            execute(storage, chain_id, block).unwrap()
        });
    Ok(num_transactions)
}

fn execute(storage: &mut Storage, chain_id: ChainId, work: ReplayWork) -> anyhow::Result<()> {
    let mut db = storage.connection()?;

    let db_tx = db.transaction().expect("Create transaction");

    let execution_state = ExecutionState::trace(&db_tx, chain_id, work.header.clone(), None);

    let mut transactions = Vec::new();
    for transaction in work.transactions {
        let transaction = pathfinder_rpc::compose_executor_transaction(&transaction, &db_tx)?;
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
    let mut cumulative_libfuncs_weight: OrderedHashMap<SmolStr, usize> = OrderedHashMap::default();
    for simulation in simulations.iter() {
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
