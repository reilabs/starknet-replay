//! This file contains the code to process a transaction trace and update the
//! hashmap which keeps the statistics of the number of calls for each libfunc.

use std::collections::HashMap;

use anyhow::bail;
use cairo_lang_runner::profiling::{
    ProfilingInfoProcessor,
    ProfilingInfoProcessorParams,
};
use cairo_lang_runner::ProfilingInfoCollectionConfig;
use cairo_lang_sierra::program::Program;
use cairo_lang_sierra_to_casm::metadata::MetadataComputationConfig;
use cairo_lang_starknet_classes::contract_class::ContractClass as CairoContractClass;
use cairo_lang_utils::ordered_hash_map::OrderedHashMap;
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use pathfinder_common::{BlockNumber, ClassHash as PathfinderClassHash};
use pathfinder_executor::types::TransactionTrace;
use pathfinder_executor::IntoFelt;
use pathfinder_rpc::v02::types::{ContractClass, SierraContractClass};
use pathfinder_storage::{BlockId, Transaction};
use smol_str::SmolStr;
use starknet_api::core::ClassHash as StarknetClassHash;
use starknet_api::hash::StarkFelt;

use crate::runner::replace_ids::replace_sierra_ids_in_program;
use crate::runner::SierraCasmRunnerLight;

/// Returns the hashmap of visited program counters for the input `trace`.
///
/// The result is a hashmap where the key is the `StarknetClassHash` and the
/// value is the Vector of visited program counters for each `StarknetClassHash`
/// execution in `trace`.
/// If `trace` is not an Invoke transaction, it returns None.
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

/// Return the `ContractClass` object of a `class_hash` at `block_num` from the
/// Pathfinder database `db`.
///
/// # Arguments
///
/// - `block_num`: The block number at which to retrieve the `ContractClass`.
/// - `db`: The connection with the Pathfinder database.
/// - `class_hash`: The class hash of the `ContractClass` to return
///
/// # Errors
///
/// Returns [`Err`] if `class_hash` doesn't exist at block `block_num` in `db`.
fn get_class_definition_at_block(
    block_num: BlockNumber,
    db: &Transaction,
    class_hash: &StarknetClassHash,
) -> anyhow::Result<ContractClass> {
    let block_id = BlockId::Number(block_num);
    let class_hash: StarkFelt = class_hash.0;
    let class_definition = db.class_definition_at(
        block_id,
        PathfinderClassHash(class_hash.into_felt()),
    );
    let class_definition = class_definition?.unwrap();

    ContractClass::from_definition_bytes(&class_definition)
}

/// Converts `ctx` from `SierraContractClass` to `Program`.
///
/// # Arguments
///
/// - `ctx`: The input `SierraContractClass`
///
/// # Errors
///
/// Returns [`Err`] if there is a serde deserialisation issue.
fn get_sierra_program_from_class_definition(
    ctx: &SierraContractClass,
) -> anyhow::Result<Program> {
    let json = serde_json::json!({
        "abi": [],
        "sierra_program": ctx.sierra_program,
        "contract_class_version": ctx.contract_class_version,
        "entry_points_by_type": ctx.entry_points_by_type,
    });
    let contract_class: CairoContractClass =
        serde_json::from_value::<CairoContractClass>(json)?;
    let sierra_program = contract_class.extract_sierra_program()?;
    let sierra_program = replace_sierra_ids_in_program(&sierra_program);
    Ok(sierra_program)
}

/// Construct the default configuration for the profiler.
///
/// To collect the list of libfunc being used during contract invocation, we
/// only need to know the `concrete_libfunc` or the `generic_libfunc`.
/// `concrete_libfunc` differentiates between different instantiations of a
/// generic type, unlike `generic_libfunc`.
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

/// Update `cumulative_libfuncs_weight` with the frequency of libfuncs called in
/// transaction `trace`.
///
/// To extract the libfuncs called, it needs to query the Starknet contract from
/// the Pathfinder database. Then, it runs the profiler over the transaction
/// `trace`.
///
/// # Arguments
///
/// - `trace`: The transaction analysed.
/// - `block_num`: The block where `trace` is inserted in.
/// - `db`: This is the open `Transaction` with the `pathfinder` database.
/// - `cumulative_libfuncs_weight`: This is a hashmap where the key is the
///   libfunc name and the value is the number of times the key has been called.
///   If the libfunc is never called, it'a not present. The value is increased
///   if the key is already present.
pub fn extract_libfuncs_weight(
    trace: &TransactionTrace,
    block_num: BlockNumber,
    db: &Transaction,
) -> anyhow::Result<OrderedHashMap<SmolStr, usize>> {
    let Some(visited_pcs) = get_visited_program_counters(trace) else {
        bail!("Error getting visited program counters from trace")
    };

    let mut local_cumulative_libfuncs_weight: OrderedHashMap<SmolStr, usize> =
        OrderedHashMap::default();
    for (class_hash, all_pcs) in visited_pcs {
        // First get the class_definition from the db using the class_hash
        let Ok(ContractClass::Sierra(ctx)) =
            get_class_definition_at_block(block_num, db, class_hash)
        else {
            continue;
        };

        // Second from the class_definition, generate the sierra_program
        let Ok(sierra_program) = get_sierra_program_from_class_definition(&ctx)
        else {
            continue;
        };

        // Third setup the runner
        let runner = SierraCasmRunnerLight::new(
            sierra_program.clone(),
            Some(MetadataComputationConfig::default()),
            Some(ProfilingInfoCollectionConfig::default()),
        )
        .unwrap();

        // Fourth iterate through each run of the contract
        for pcs in all_pcs {
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
                continue;
            };

            let profiling_info_processor_params =
                get_profiling_info_processor_params();
            let profiling_info = profiling_info_processor.process_ex(
                &raw_profiling_info,
                &profiling_info_processor_params,
            );
            let Some(concrete_libfunc_weights) =
                profiling_info.libfunc_weights.concrete_libfunc_weights
            else {
                continue;
            };
            concrete_libfunc_weights
                .iter()
                .for_each(|(libfunc, weight)| {
                    local_cumulative_libfuncs_weight
                        .entry(libfunc.clone())
                        .and_modify(|e| *e += *weight)
                        .or_insert(*weight);
                });
        }
    }
    Ok(local_cumulative_libfuncs_weight)
}

#[cfg(test)]
mod tests {
    use std::{env, fs, io};

    use itertools::Itertools;

    use super::*;

    fn read_test_file(filename: &str) -> io::Result<String> {
        let out_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
        let sierra_program_json_file =
            [out_dir.as_str(), filename].iter().join("");
        let sierra_program_json_file = sierra_program_json_file.as_str();
        fs::read_to_string(sierra_program_json_file)
    }

    #[test]
    fn test_get_profiling_info_processor_params() {
        // Checking that the important settings are setup correctly to generate
        // a histogram of libfuncs frequency.
        let profiling_info_processor_params =
            get_profiling_info_processor_params();
        assert_eq!(profiling_info_processor_params.min_weight, 1);
        assert!(profiling_info_processor_params.process_by_concrete_libfunc);
    }

    #[test]
    fn test_get_sierra_program_from_class_definition() {
        let sierra_program_json_file = "/test_data/sierra_felt.json";
        let sierra_program_json = read_test_file(sierra_program_json_file)
            .unwrap_or_else(|_| {
                panic!("Unable to read file {sierra_program_json_file}")
            });
        let sierra_program_json: serde_json::Value =
            serde_json::from_str(&sierra_program_json).unwrap_or_else(|_| {
                panic!("Unable to parse {sierra_program_json_file} to json")
            });
        let contract_class: SierraContractClass =
            serde_json::from_value::<SierraContractClass>(sierra_program_json)
                .unwrap_or_else(|_| {
                    panic!(
                        "Unable to parse {sierra_program_json_file} to \
                         SierraContractClass"
                    )
                });
        let sierra_program =
            get_sierra_program_from_class_definition(&contract_class)
                .unwrap_or_else(|_| {
                    panic!(
                        "Unable to create Program {sierra_program_json_file} \
                         to SierraContractClass"
                    )
                });

        let sierra_program_test_file = "/test_data/sierra_program.json";
        let sierra_program_test_json = read_test_file(sierra_program_test_file)
            .unwrap_or_else(|_| {
                panic!("Unable to read file {sierra_program_test_file}")
            });
        let sierra_program_test_json: serde_json::Value = serde_json::from_str(
            &sierra_program_test_json,
        )
        .unwrap_or_else(|_| {
            panic!("Unable to parse {sierra_program_test_file} to json")
        });
        let sierra_program_test: Program =
            serde_json::from_value::<Program>(sierra_program_test_json)
                .unwrap_or_else(|_| {
                    panic!(
                        "Unable to parse {sierra_program_test_file} to Program"
                    )
                });

        assert_eq!(sierra_program_test, sierra_program);
    }
}
