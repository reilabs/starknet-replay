use std::num::NonZeroU32;
use std::path::PathBuf;

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

#[derive(Debug, Clone, Eq, PartialEq)]
struct Work {
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
    let storage = Storage::migrate(db_path.into(), JournalMode::WAL, 1)?
        .create_pool(NonZeroU32::new(n_cpus as u32 * 2).unwrap())?;
    let mut db = storage
        .connection()
        .context("Opening database connection")?;

    (start_block..=end_block)
        .map(|block_number| {
            let transaction = db.transaction().unwrap();
            let block_id = BlockId::Number(BlockNumber::new_or_panic(block_number));
            let block_header = transaction.block_header(block_id).unwrap().unwrap();
            let transactions_and_receipts = transaction
                .transaction_data_for_block(block_id)
                .unwrap()
                .context("Getting transactions for block")
                .unwrap();
            drop(transaction);

            let (transactions, receipts): (Vec<_>, Vec<_>) =
                transactions_and_receipts.into_iter().unzip();

            num_transactions += transactions.len();

            Work {
                header: block_header,
                transactions,
                receipts,
            }
        })
        .par_bridge()
        .for_each_with(storage, |storage, block| execute(storage, chain_id, block));
    Ok(num_transactions)
}

fn execute(storage: &mut Storage, chain_id: ChainId, work: Work) {
    let start_time = std::time::Instant::now();
    let num_transactions = work.transactions.len();

    let mut db = storage.connection().unwrap();

    let db_tx = db.transaction().expect("Create transaction");

    let execution_state = ExecutionState::trace(&db_tx, chain_id, work.header.clone(), None);

    let transactions = work
        .transactions
        .into_iter()
        .map(|tx| {
            let tx = pathfinder_rpc::compose_executor_transaction(&tx, &db_tx);
            tx
        })
        .collect::<Result<Vec<_>, _>>();

    let transactions = match transactions {
        Ok(transactions) => transactions,
        Err(error) => {
            tracing::error!(block_number=%work.header.number, %error, "Transaction conversion failed");
            return;
        }
    };

    match pathfinder_executor::simulate(execution_state, transactions, false, false) {
        Ok(simulations) => {
            let mut cumulative_libfuncs_weight: OrderedHashMap<SmolStr, usize> =
                OrderedHashMap::default();
            for (simulation, receipt) in simulations.iter().zip(work.receipts.iter()) {
                if let Some(actual_fee) = receipt.actual_fee {
                    let actual_fee =
                        u128::from_be_bytes(actual_fee.0.to_be_bytes()[16..].try_into().unwrap());

                    // L1 handler transactions have a fee of zero in the receipt.
                    if actual_fee == 0 {
                        continue;
                    }

                    let estimate = &simulation.fee_estimation;

                    let (gas_price, data_gas_price) = match estimate.unit {
                        pathfinder_executor::types::PriceUnit::Wei => (
                            work.header.eth_l1_gas_price.0,
                            work.header.eth_l1_data_gas_price.0,
                        ),
                        pathfinder_executor::types::PriceUnit::Fri => (
                            work.header.strk_l1_gas_price.0,
                            work.header.strk_l1_data_gas_price.0,
                        ),
                    };

                    let actual_data_gas_consumed =
                        receipt.execution_resources.data_availability.l1_data_gas;
                    let actual_gas_consumed = (actual_fee
                        - actual_data_gas_consumed.saturating_mul(data_gas_price))
                        / gas_price.max(1);

                    let estimated_gas_consumed = estimate.gas_consumed.as_u128();
                    let estimated_data_gas_consumed = estimate.data_gas_consumed.as_u128();

                    let gas_diff = actual_gas_consumed.abs_diff(estimated_gas_consumed);
                    let data_gas_diff =
                        actual_data_gas_consumed.abs_diff(estimated_data_gas_consumed);

                    if gas_diff > (actual_gas_consumed * 2 / 10)
                        || data_gas_diff > (actual_data_gas_consumed * 2 / 10)
                    {
                        tracing::warn!(block_number=%work.header.number, transaction_hash=%receipt.transaction_hash, %estimated_gas_consumed, %actual_gas_consumed, %estimated_data_gas_consumed, %actual_data_gas_consumed, estimated_fee=%estimate.overall_fee, %actual_fee, "Estimation mismatch");
                    } else {
                        tracing::debug!(block_number=%work.header.number, transaction_hash=%receipt.transaction_hash, %estimated_gas_consumed, %actual_gas_consumed, %estimated_data_gas_consumed, %actual_data_gas_consumed, estimated_fee=%estimate.overall_fee, %actual_fee, "Estimation matches");
                    }
                }

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
        }
        Err(error) => {
            tracing::error!(block_number=%work.header.number, ?error, "Transaction re-execution failed");
        }
    }

    let elapsed = start_time.elapsed().as_millis();

    tracing::debug!(block_number=%work.header.number, %num_transactions, %elapsed, "Re-executed block");
}
