use std::collections::HashMap;

use cairo_lang_runner::profiling::{ProfilingInfoProcessor, ProfilingInfoProcessorParams};
use cairo_lang_runner::ProfilingInfoCollectionConfig;
use cairo_lang_sierra::program::Program;
use cairo_lang_starknet_classes::contract_class::ContractClass as CairoContractClass;
use cairo_lang_utils::ordered_hash_map::OrderedHashMap;
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use pathfinder_common::BlockNumber;
use pathfinder_executor::types::TransactionTrace;
use pathfinder_executor::IntoFelt;
use pathfinder_rpc::v02::types::{ContractClass, SierraContractClass};
use pathfinder_storage::{BlockId, Transaction};
use smol_str::SmolStr;
use starknet_api::hash::StarkFelt;

use crate::runner::replace_ids::replace_sierra_ids_in_program;
use crate::runner::SierraCasmRunnerLight;

fn get_visited_pcs(
    trace: &TransactionTrace,
) -> Option<&HashMap<starknet_api::core::ClassHash, Vec<Vec<usize>>>> {
    match trace {
        TransactionTrace::Invoke(tx) => Some(&tx.visited_pcs),
        _ => None,
    }
}

fn get_class_definition_at_block(
    block_num: BlockNumber,
    db: &Transaction,
    class_hash: &starknet_api::core::ClassHash,
) -> anyhow::Result<ContractClass> {
    let block_id = BlockId::Number(block_num);
    let class_hash: StarkFelt = class_hash.0;
    let class_definition = db.class_definition_at(
        block_id,
        pathfinder_common::ClassHash(class_hash.into_felt()),
    );
    let class_definition = class_definition?.unwrap();

    ContractClass::from_definition_bytes(&class_definition)
}

fn get_sierra_program_from_class_definition(ctx: SierraContractClass) -> anyhow::Result<Program> {
    let json = serde_json::json!({
        "abi": [],
        "sierra_program": ctx.sierra_program,
        "contract_class_version": ctx.contract_class_version,
        "entry_points_by_type": ctx.entry_points_by_type,
    });
    let contract_class: CairoContractClass = serde_json::from_value::<CairoContractClass>(json)?;
    let sierra_program = contract_class.extract_sierra_program()?;
    let sierra_program = replace_sierra_ids_in_program(&sierra_program);
    Ok(sierra_program)
}

fn get_profiling_info_processor_params() -> ProfilingInfoProcessorParams {
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
    }
}

pub fn analyse_tx(
    trace: &TransactionTrace,
    block_num: BlockNumber,
    db: &Transaction,
    cumulative_libfuncs_weight: &mut OrderedHashMap<SmolStr, usize>,
) {
    let Some(visited_pcs) = get_visited_pcs(trace) else {
        return;
    };

    visited_pcs.iter().for_each(|(class_hash, all_pcs)| {
        // First get the class_definition from the db using the class_hash
        let Ok(ContractClass::Sierra(ctx)) =
            get_class_definition_at_block(block_num, db, class_hash)
        else {
            return;
        };

        // Second from the class_definition, generate the sierra_program
        let Ok(sierra_program) = get_sierra_program_from_class_definition(ctx) else {
            return;
        };

        // Third setup the runner
        let runner = SierraCasmRunnerLight::new(
            sierra_program.clone(),
            Some(Default::default()),
            Some(ProfilingInfoCollectionConfig::default()),
        )
        .unwrap();

        // Fourth iterate through each run of the contract
        all_pcs.iter().for_each(|pcs| {
            let raw_profiling_info = runner
                .run_profiler
                .as_ref()
                .map(|_| runner.collect_profiling_info(pcs.as_slice()));

            let profiling_info_processor = ProfilingInfoProcessor::new(
                None,
                sierra_program.clone(),
                UnorderedHashMap::default(),
            );
            let Some(raw_profiling_info) = raw_profiling_info else {
                return;
            };

            let profiling_info_processor_params = get_profiling_info_processor_params();
            let profiling_info = profiling_info_processor
                .process_ex(&raw_profiling_info, &profiling_info_processor_params);
            profiling_info
                .libfunc_weights
                .concrete_libfunc_weights
                .unwrap()
                .iter()
                .for_each(|(libfunc, weight)| {
                    cumulative_libfuncs_weight
                        .entry(libfunc.clone())
                        .and_modify(|e| *e += *weight)
                        .or_insert(*weight);
                });
        });
    });
}