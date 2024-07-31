//! This file contains the code to process a transaction trace and update the
//! hashmap which keeps the statistics of the number of calls for each libfunc.

use cairo_lang_runner::profiling::{ProfilingInfoProcessor, ProfilingInfoProcessorParams};
use cairo_lang_sierra::program::Program;
use cairo_lang_starknet_classes::contract_class::ContractClass as CairoContractClass;
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use itertools::Itertools;
use starknet::core::types::ContractClass;

use crate::profiler::replace_ids::replace_sierra_ids_in_program;
use crate::profiler::replay_statistics::ReplayStatistics;
use crate::profiler::{ProfilerError, SierraProfiler};
use crate::runner::replay_class_hash::VisitedPcs;
use crate::storage::Storage;

/// Converts transforms a [`starknet::core::types::ContractClass`] in Sierra
/// [`cairo_lang_sierra::program::Program`].
///
/// # Arguments
///
/// - `ctx`: The input [`starknet::core::types::ContractClass`]
///
/// # Errors
///
/// Returns [`Err`] if there is a serde deserialisation issue.
fn get_sierra_program_from_class_definition(ctx: &ContractClass) -> Result<Program, ProfilerError> {
    match ctx {
        ContractClass::Sierra(ctx) => {
            let mut json = serde_json::to_value(ctx)?;
            json.as_object_mut()
                .ok_or(ProfilerError::Unknown(
                    "Failed serialising `ContractClass`.".to_string(),
                ))?
                .remove("abi");
            let contract_class: CairoContractClass =
                serde_json::from_value::<CairoContractClass>(json)?;
            // TODO: `extract_sierra_program` returns an error of type `Felt252SerdeError`
            // which is private. For ease of integration with `thiserror`, it needs to be
            // made public. Issue #20
            let sierra_program = contract_class.extract_sierra_program().map_err(|_| {
                ProfilerError::Unknown("Error extracting sierra program".to_string().to_string())
            })?;
            let sierra_program = replace_sierra_ids_in_program(&sierra_program);
            Ok(sierra_program)
        }
        ContractClass::Legacy(_) => {
            Err(ProfilerError::Unknown("Not a Sierra contract.".to_string()))
        }
    }
}

/// Constructs the default configuration for the profiler.
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

/// Extracts the frequency of libfuncs from visited program counters.
///
/// The process to extract the frequency of libfuncs called is:
/// 1- Get the vector of visited program counters
/// 2- Query the RPC endpoint to extract the Starknet contract from the class
/// hash and block number.
/// 3- Run the profiler over the list of visited program
/// counters to determine which lines of the Sierra code have been executed and
/// collect the results.
///
/// # Arguments
///
/// - `visited_pcs`: The object that contains the list of visited program
///   counters for each transaction replayed.
/// - `storage`: The object to query the starknet blockchain using the RPC
///   protocol.
///
/// # Errors
///
/// Returns [`Err`] if the constructor of `SierraCasmRunnerLight` fails.
pub fn extract_libfuncs_weight(
    visited_pcs: &VisitedPcs,
    storage: &impl Storage,
) -> Result<ReplayStatistics, ProfilerError> {
    let mut local_cumulative_libfuncs_weight: ReplayStatistics = ReplayStatistics::new();

    for (replay_class_hash, all_pcs) in visited_pcs {
        let Ok(contract_class) = storage.get_contract_class_at_block(replay_class_hash) else {
            continue;
        };

        let Ok(sierra_program) = get_sierra_program_from_class_definition(&contract_class) else {
            continue;
        };

        let runner = SierraProfiler::new(sierra_program.clone())?;

        for pcs in all_pcs {
            let raw_profiling_info = runner.collect_profiling_info(pcs.as_slice())?;

            let profiling_info_processor = ProfilingInfoProcessor::new(
                None,
                sierra_program.clone(),
                UnorderedHashMap::default(),
            );

            let profiling_info_processor_params = get_profiling_info_processor_params();
            let profiling_info = profiling_info_processor
                .process_ex(&raw_profiling_info, &profiling_info_processor_params);
            let Some(concrete_libfunc_weights) =
                profiling_info.libfunc_weights.concrete_libfunc_weights
            else {
                continue;
            };
            local_cumulative_libfuncs_weight =
                local_cumulative_libfuncs_weight.add_statistics(&concrete_libfunc_weights);
        }
    }

    for (concrete_name, weight) in local_cumulative_libfuncs_weight
        .concrete_libfunc
        .iter()
        .sorted_by(|a, b| Ord::cmp(&a.1, &b.1))
    {
        tracing::info!("  libfunc {concrete_name}: {weight}");
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
        let sierra_program_json_file = [out_dir.as_str(), filename].iter().join("");
        let sierra_program_json_file = sierra_program_json_file.as_str();
        fs::read_to_string(sierra_program_json_file)
    }

    #[test]
    fn test_get_profiling_info_processor_params() {
        // Checking that the important settings are setup correctly to generate
        // a histogram of libfuncs frequency.
        let profiling_info_processor_params = get_profiling_info_processor_params();
        assert_eq!(profiling_info_processor_params.min_weight, 1);
        assert!(profiling_info_processor_params.process_by_concrete_libfunc);
    }

    #[test]
    fn test_get_sierra_program_from_class_definition() {
        let sierra_program_json_file = "/test_data/sierra_felt.json";
        let sierra_program_json = read_test_file(sierra_program_json_file)
            .unwrap_or_else(|_| panic!("Unable to read file {sierra_program_json_file}"));
        let sierra_program_json: serde_json::Value = serde_json::from_str(&sierra_program_json)
            .unwrap_or_else(|_| panic!("Unable to parse {sierra_program_json_file} to json"));
        let contract_class: ContractClass =
            serde_json::from_value::<ContractClass>(sierra_program_json).unwrap_or_else(|_| {
                panic!("Unable to parse {sierra_program_json_file} to SierraContractClass")
            });
        let sierra_program = get_sierra_program_from_class_definition(&contract_class)
            .unwrap_or_else(|_| {
                panic!("Unable to create Program {sierra_program_json_file} to SierraContractClass")
            });

        let sierra_program_test_file = "/test_data/sierra_program.json";
        let sierra_program_test_json = read_test_file(sierra_program_test_file)
            .unwrap_or_else(|_| panic!("Unable to read file {sierra_program_test_file}"));
        let sierra_program_test_json: serde_json::Value =
            serde_json::from_str(&sierra_program_test_json)
                .unwrap_or_else(|_| panic!("Unable to parse {sierra_program_test_file} to json"));
        let sierra_program_test: Program =
            serde_json::from_value::<Program>(sierra_program_test_json).unwrap_or_else(|_| {
                panic!("Unable to parse {sierra_program_test_file} to Program")
            });

        assert_eq!(sierra_program_test, sierra_program);
    }
}
