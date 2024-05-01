use crate::profiler::replace_sierra_ids_in_program;
use crate::runner::SierraCasmRunnerLight;
use cairo_lang_runner::profiling::{ProfilingInfoProcessor, ProfilingInfoProcessorParams};
use cairo_lang_runner::ProfilingInfoCollectionConfig;
use cairo_lang_starknet_classes::contract_class::ContractClass as CairoContractClass;
use cairo_lang_utils::ordered_hash_map::OrderedHashMap;
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use itertools::Itertools;
use pathfinder_common::receipt::Receipt;
use pathfinder_common::transaction::Transaction;
use pathfinder_common::{BlockHeader, ChainId};
use pathfinder_executor::types::TransactionTrace;
use pathfinder_executor::{ExecutionState, IntoFelt};
use pathfinder_rpc::v02::types::ContractClass;
use pathfinder_storage::{BlockId, Storage};
use smol_str::SmolStr;
use starknet_api::hash::StarkFelt;

mod profiler;
mod runner;

#[derive(Debug)]
pub struct Work {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
    pub receipts: Vec<Receipt>,
}

pub fn execute(storage: &mut Storage, chain_id: ChainId, work: Work) {
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

                match &simulation.trace {
                    TransactionTrace::Invoke(tx) => {
                        let visited_pcs = &tx.visited_pcs;
                        visited_pcs.iter().for_each(|(class_hash, pcs)| {
                            // First get the class_definition from the db using the class_hash
                            let block_id = BlockId::Number(work.header.number);
                            let class_hash: StarkFelt = class_hash.clone().0.into();
                            let class_definition = db_tx.class_definition_at(
                                block_id,
                                pathfinder_common::ClassHash(class_hash.into_felt()),
                            );
                            let class_definition = class_definition.unwrap().unwrap();
                            let class_definition =
                                ContractClass::from_definition_bytes(&class_definition);

                            // Second from the class_definition, generate the sierra_program
                            match class_definition {
                                Ok(ContractClass::Sierra(ctx)) => {
                                    let json = serde_json::json!({
                                        "abi": [],
                                        "sierra_program": ctx.sierra_program,
                                        "contract_class_version": ctx.contract_class_version,
                                        "entry_points_by_type": ctx.entry_points_by_type,
                                    });
                                    let contract_class: CairoContractClass =
                                        serde_json::from_value::<CairoContractClass>(json).unwrap();
                                    let sierra_program =
                                        contract_class.extract_sierra_program().unwrap();

                                    let sierra_program =
                                        replace_sierra_ids_in_program(&sierra_program);

                                    // Third setup the runner
                                    let runner = SierraCasmRunnerLight::new(
                                        sierra_program.clone(),
                                        Some(Default::default()),
                                        Some(ProfilingInfoCollectionConfig::default()),
                                    )
                                    .unwrap();

                                    for run_pcs in pcs {
                                        let profiling_info =
                                            runner.run_profiler.as_ref().map(|_| {
                                                runner.collect_profiling_info(run_pcs.as_slice())
                                            });

                                        let profiling_info_processor = ProfilingInfoProcessor::new(
                                            None,
                                            sierra_program.clone(),
                                            UnorderedHashMap::default(),
                                        );
                                        match profiling_info {
                                            Some(raw_profiling_info) => {
                                                let profiling_info_processor_params =
                                                    ProfilingInfoProcessorParams {
                                                        min_weight: 1,
                                                        process_by_statement: false,
                                                        process_by_concrete_libfunc: true,
                                                        process_by_generic_libfunc: false,
                                                        process_by_user_function: false,
                                                        process_by_original_user_function: false,
                                                        process_by_cairo_function: false,
                                                        process_by_stack_trace: false,
                                                        process_by_cairo_stack_trace: false,
                                                    };
                                                let profiling_info = profiling_info_processor
                                                    .process_ex(
                                                        &raw_profiling_info,
                                                        &profiling_info_processor_params,
                                                    );
                                                profiling_info
                                                    .libfunc_weights
                                                    .concrete_libfunc_weights
                                                    .unwrap()
                                                    .iter()
                                                    .for_each(|(libfunc, weight)| {
                                                        cumulative_libfuncs_weight
                                                            .entry(libfunc.clone())
                                                            .and_modify(|e| *e += weight.clone())
                                                            .or_insert(weight.clone());
                                                    });
                                            }
                                            None => {
                                                println!("Warning: Profiling info not found.")
                                            }
                                        }
                                    }
                                }
                                _ => (),
                            }
                        });
                    }
                    _ => (),
                }
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
