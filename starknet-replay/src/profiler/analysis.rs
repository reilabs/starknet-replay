//! This file contains the code to process a transaction trace and update the
//! hashmap which keeps the statistics of the number of calls for each libfunc.

use std::collections::HashMap;

use cairo_lang_sierra::program::Program;
use cairo_lang_starknet_classes::contract_class::ContractClass as CairoContractClass;
use itertools::Itertools;
use starknet_core::types::ContractClass;

use crate::profiler::replace_ids::replace_sierra_ids_in_program;
use crate::profiler::replay_statistics::ReplayStatistics;
use crate::profiler::{ProfilerError, SierraProfiler};
use crate::runner::replay_class_hash::VisitedPcs;
use crate::storage::Storage;

/// Converts transforms a [`starknet_core::types::ContractClass`] in Sierra
/// [`cairo_lang_sierra::program::Program`].
///
/// # Arguments
///
/// - `ctx`: The input [`starknet_core::types::ContractClass`]
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

/// Returns the frequency of libfuncs for a given Sierra contract.
///
/// # Arguments
///
/// - `runner`: The Sierra profiler object.
/// - `pcs`: The vector of program counters from each execution of the Sierra
///   contract in `runner`.
fn internal_extract_libfuncs_weight(
    runner: &SierraProfiler,
    pcs: &Vec<usize>,
) -> HashMap<String, usize> {
    let raw_profiling_info = runner.collect_profiling_info(pcs.as_slice());
    runner.unpack_profiling_info(&raw_profiling_info)
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
    let mut local_cumulative_libfuncs_weight = ReplayStatistics::new();

    for (replay_class_hash, all_pcs) in visited_pcs {
        tracing::info!("Processing pcs from {replay_class_hash:?}.");
        let Ok(contract_class) = storage.get_contract_class_at_block(replay_class_hash) else {
            continue;
        };

        let Ok(sierra_program) = get_sierra_program_from_class_definition(&contract_class) else {
            continue;
        };

        let runner = SierraProfiler::new(sierra_program, None)?;

        let concrete_libfunc_weights = internal_extract_libfuncs_weight(&runner, all_pcs);

        local_cumulative_libfuncs_weight =
            local_cumulative_libfuncs_weight.add_statistics(&concrete_libfunc_weights);
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
    use std::path::Path;
    use std::process::Command;
    use std::{env, fs, io};

    use cairo_lang_compiler::{compile_cairo_project_at_path, CompilerConfig};
    use cairo_lang_starknet::compile::compile_path;
    use cairo_lang_starknet_classes::casm_contract_class::CasmContractClass;
    use cairo_vm::hint_processor::cairo_1_hint_processor::hint_processor::Cairo1HintProcessor;
    use cairo_vm::types::builtin_name::BuiltinName;
    use cairo_vm::types::layout_name::LayoutName;
    use cairo_vm::types::relocatable::MaybeRelocatable;
    use cairo_vm::vm::runners::cairo_runner::{CairoArg, CairoRunner, RunResources};
    use itertools::Itertools;

    use super::*;

    fn read_file_to_string(filename: &str) -> io::Result<String> {
        let out_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
        let sierra_program_json_file = [out_dir.as_str(), filename].iter().join("");
        let sierra_program_json_file = sierra_program_json_file.as_str();
        fs::read_to_string(sierra_program_json_file)
    }

    fn read_sierra_compressed_program(filename: &str) -> Program {
        let sierra_program_json = read_file_to_string(filename)
            .unwrap_or_else(|_| panic!("Unable to read file {filename}"));
        let sierra_program_json: serde_json::Value = serde_json::from_str(&sierra_program_json)
            .unwrap_or_else(|_| panic!("Unable to parse {filename} to json"));
        let contract_class: ContractClass = serde_json::from_value(sierra_program_json)
            .unwrap_or_else(|_| panic!("Unable to parse {filename} to SierraContractClass"));
        get_sierra_program_from_class_definition(&contract_class).unwrap_or_else(|_| {
            panic!("Unable to create Program {filename} to SierraContractClass")
        })
    }

    fn read_sierra_program(filename: &str) -> Program {
        let sierra_program_test_json = read_file_to_string(filename)
            .unwrap_or_else(|_| panic!("Unable to read file {filename}"));
        let sierra_program_test_json: serde_json::Value =
            serde_json::from_str(&sierra_program_test_json)
                .unwrap_or_else(|_| panic!("Unable to parse {filename} to json"));
        serde_json::from_value(sierra_program_test_json)
            .unwrap_or_else(|_| panic!("Unable to parse {filename} to Program"))
    }

    fn assert_libfunc_frequency(statistics: &HashMap<String, usize>, name: &str, frequency: usize) {
        let value = statistics.get(&name.to_string()).unwrap();
        assert_eq!(
            *value, frequency,
            "Frequency for {}. Expected {}, found {}",
            name, frequency, *value
        );
    }

    // This function uses `ctor` because it must be called only once before starting
    // unit testing to download corelib.
    #[ctor::ctor]
    fn download_corelib() {
        let project_root_path = env::var("CARGO_MANIFEST_DIR").unwrap();
        let corelib_directory = [project_root_path.as_str(), "/corelib"].iter().join("");
        if !fs::exists(corelib_directory).unwrap() {
            Command::new("make")
                .current_dir(project_root_path)
                .args(["deps"])
                .status()
                .unwrap();
        }
    }

    fn compile_cairo_program(filename: &str, replace_ids: bool) -> Program {
        let absolute_path = env::var("CARGO_MANIFEST_DIR").unwrap();
        let filename = [absolute_path.as_str(), filename].iter().join("");
        let file_path = Path::new(&filename);
        compile_cairo_project_at_path(
            file_path,
            CompilerConfig {
                replace_ids,
                ..CompilerConfig::default()
            },
        )
        .unwrap()
    }

    fn compile_cairo_contract(
        filename: &str,
        replace_ids: bool,
    ) -> cairo_lang_starknet_classes::contract_class::ContractClass {
        let absolute_path = env::var("CARGO_MANIFEST_DIR").unwrap();
        let filename = [absolute_path.as_str(), filename].iter().join("");
        let file_path = Path::new(&filename);
        let config = CompilerConfig {
            replace_ids,
            ..CompilerConfig::default()
        };
        let contract_path = None;
        compile_path(file_path, contract_path, config).unwrap()
    }

    fn get_casm_contract_builtins(
        contract_class: &CasmContractClass,
        entrypoint_offset: usize,
    ) -> Vec<BuiltinName> {
        contract_class
            .entry_points_by_type
            .external
            .iter()
            .find(|e| e.offset == entrypoint_offset)
            .unwrap()
            .builtins
            .iter()
            .map(|s| BuiltinName::from_str(s).expect("Invalid builtin name"))
            .collect()
    }

    #[allow(clippy::too_many_lines)] // Allowed because this is a test
    fn visited_pcs_from_entrypoint(
        filename: &str,
        entrypoint_offset: usize,
        args: &[MaybeRelocatable],
    ) -> Vec<usize> {
        let contract_class = compile_cairo_contract(filename, true);

        let add_pythonic_hints = false;
        let max_bytecode_size = 180_000;
        let contract_class: CasmContractClass = CasmContractClass::from_contract_class(
            contract_class,
            add_pythonic_hints,
            max_bytecode_size,
        )
        .unwrap();
        let segment_arena_validations = false;
        let mut hint_processor = Cairo1HintProcessor::new(
            &contract_class.hints,
            RunResources::default(),
            segment_arena_validations,
        );

        let proof_mode = true;
        let trace_enabled = true;
        let mut runner = CairoRunner::new(
            &(contract_class.clone().try_into().unwrap()),
            LayoutName::all_cairo,
            proof_mode,
            trace_enabled,
        )
        .unwrap();

        let program_builtins = get_casm_contract_builtins(&contract_class, entrypoint_offset);
        runner
            .initialize_function_runner_cairo_1(&program_builtins)
            .unwrap();

        // Implicit Args
        let syscall_segment = MaybeRelocatable::from(runner.vm.add_memory_segment());

        let builtins = runner.get_program_builtins();

        let builtin_segment: Vec<MaybeRelocatable> = runner
            .vm
            .get_builtin_runners()
            .iter()
            .filter(|b| builtins.contains(&b.name()))
            .flat_map(cairo_vm::vm::runners::builtin_runner::BuiltinRunner::initial_stack)
            .collect();

        let initial_gas = MaybeRelocatable::from(usize::MAX);

        let mut implicit_args = builtin_segment;
        implicit_args.extend([initial_gas]);
        implicit_args.extend([syscall_segment]);

        // Other args

        // Load builtin costs
        let builtin_costs: Vec<MaybeRelocatable> =
            vec![0.into(), 0.into(), 0.into(), 0.into(), 0.into()];
        let builtin_costs_ptr = runner.vm.add_memory_segment();
        runner
            .vm
            .load_data(builtin_costs_ptr, &builtin_costs)
            .unwrap();

        // Load extra data
        let core_program_end_ptr =
            (runner.program_base.unwrap() + runner.get_program().data_len()).unwrap();
        let program_extra_data: Vec<MaybeRelocatable> =
            vec![0x208B_7FFF_7FFF_7FFE.into(), builtin_costs_ptr.into()];
        runner
            .vm
            .load_data(core_program_end_ptr, &program_extra_data)
            .unwrap();

        // Load calldata
        let calldata_start = runner.vm.add_memory_segment();
        let calldata_end = runner.vm.load_data(calldata_start, &args.to_vec()).unwrap();

        // Create entrypoint_args

        let mut entrypoint_args: Vec<CairoArg> = implicit_args
            .iter()
            .map(|m| CairoArg::from(m.clone()))
            .collect();
        entrypoint_args.extend([
            MaybeRelocatable::from(calldata_start).into(),
            MaybeRelocatable::from(calldata_end).into(),
        ]);
        let entrypoint_args: Vec<&CairoArg> = entrypoint_args.iter().collect();

        // Run contract entrypoint
        let verify_secure = true;
        runner
            .run_from_entrypoint(
                entrypoint_offset,
                &entrypoint_args,
                verify_secure,
                Some(runner.get_program().data_len() + program_extra_data.len()),
                &mut hint_processor,
            )
            .unwrap();

        let _ = runner.relocate(true);

        let mut visited_pcs: Vec<usize> =
            Vec::with_capacity(runner.relocated_trace.as_ref().unwrap().len());

        runner
            .relocated_trace
            .as_ref()
            .unwrap()
            .iter()
            .for_each(|t| {
                let pc = t.pc;
                let real_pc = pc - 1;
                // Jumping to a PC that is not inside the bytecode is possible. For example, to
                // obtain the builtin costs. Filter out these values.
                if real_pc < runner.get_program().data_len() {
                    visited_pcs.push(pc);
                }
            });
        visited_pcs
    }

    #[test]
    fn test_get_sierra_program_from_class_definition() {
        let sierra_program_json_file = "/test_data/sierra_felt.json";
        let sierra_program = read_sierra_compressed_program(sierra_program_json_file);

        let sierra_program_test_file = "/test_data/sierra_program.json";
        let sierra_program_test = read_sierra_program(sierra_program_test_file);

        assert_eq!(sierra_program_test, sierra_program);
    }

    #[test]
    fn test_extract_libfuncs_program() {
        // This is the original CAIRO code.
        // use core::felt252;
        // fn main() -> felt252 {
        //     let n = 2 + 3;
        //     n
        // }

        // These PCs have been extracted from execution of the program in `cairo-vm`
        // unit tests.
        let visited_pcs: Vec<usize> = vec![1, 4, 6, 8, 3];

        let cairo_file = "/test_data/sierra_add_program.cairo";
        let sierra_program = compile_cairo_program(cairo_file, true);

        // The offset of the program is 4 because the header contains 3 instructions.
        let sierra_profiler = SierraProfiler::new(sierra_program.clone(), Some(4)).unwrap();

        let concrete_libfunc_weights =
            internal_extract_libfuncs_weight(&sierra_profiler, &visited_pcs);

        assert_eq!(concrete_libfunc_weights.len(), 4);
        assert_libfunc_frequency(&concrete_libfunc_weights, "store_temp<felt252>", 2);
        assert_libfunc_frequency(
            &concrete_libfunc_weights,
            "const_as_immediate<Const<felt252, 2>>",
            1,
        );
        assert_libfunc_frequency(
            &concrete_libfunc_weights,
            "const_as_immediate<Const<felt252, 3>>",
            1,
        );
        assert_libfunc_frequency(&concrete_libfunc_weights, "felt252_add", 1);

        let libfuncs = sierra_profiler.get_libfuncs_at_pc(4);
        assert_eq!(libfuncs.len(), 3);
        assert!(libfuncs.contains(&"const_as_immediate<Const<felt252, 2>>".to_string()));
        assert!(libfuncs.contains(&"const_as_immediate<Const<felt252, 3>>".to_string()));
        assert!(libfuncs.contains(&"store_temp<felt252>".to_string()));

        let libfuncs = sierra_profiler.get_libfuncs_at_pc(6);
        assert_eq!(libfuncs.len(), 2);
        assert!(libfuncs.contains(&"felt252_add".to_string()));
        assert!(libfuncs.contains(&"store_temp<felt252>".to_string()));
    }

    #[test]
    fn test_extract_libfuncs_secp_program() {
        let visited_pcs: Vec<usize> = vec![
            1, 3, 5, 8, 10, 12, 14, 16, 18, 19, 20, 21, 22, 23, 24, 25, 27, 28, 29, 30, 32, 34, 36,
            38, 40, 41, 42, 43, 44, 45, 46, 48, 49, 51, 53, 55, 57, 7,
        ];

        let cairo_file = "/test_data/sierra_secp.cairo";
        let sierra_program = compile_cairo_program(cairo_file, true);

        let sierra_profiler = SierraProfiler::new(sierra_program.clone(), Some(8)).unwrap();

        let concrete_libfunc_weights =
            internal_extract_libfuncs_weight(&sierra_profiler, &visited_pcs);

        assert_eq!(concrete_libfunc_weights.len(), 16);
        assert_libfunc_frequency(&concrete_libfunc_weights, "secp256r1_new_syscall", 1);
        assert_libfunc_frequency(&concrete_libfunc_weights, "secp256r1_mul_syscall", 1);
        assert_libfunc_frequency(&concrete_libfunc_weights, "drop<Secp256r1Point>", 1);

        let libfuncs = sierra_profiler.get_libfuncs_at_pc(16);
        assert_eq!(libfuncs.len(), 1);
        assert!(libfuncs.contains(&"secp256r1_new_syscall".to_string()));

        let libfuncs = sierra_profiler.get_libfuncs_at_pc(38);
        assert_eq!(libfuncs.len(), 1);
        assert!(libfuncs.contains(&"secp256r1_mul_syscall".to_string()));
    }

    #[test]
    fn test_extract_libfuncs_contract() {
        // This is the original CAIRO code.
        // #[starknet::interface]
        // trait HelloStarknetTrait<TContractState> {
        //     fn increase_balance(ref self: TContractState) -> felt252;
        // }
        // #[starknet::contract]
        // mod hello_starknet {
        //     #[storage]
        //     struct Storage {
        //     }
        //     #[abi(embed_v0)]
        //     impl HelloStarknetImpl of super::HelloStarknetTrait<ContractState> {
        //         fn increase_balance(ref self: ContractState) -> felt252 {
        //             let a = 7 + 11;
        //             let b = a + 13;
        //             let c = b + 49;
        //             let d = c + 17;
        //             let e = d + 19;
        //             let f = e + 23;
        //             let g = f + 31;
        //             g
        //         }
        //     }
        // }

        let cairo_file = "/test_data/sierra_add_contract.cairo";
        let sierra_program = compile_cairo_contract(cairo_file, true)
            .extract_sierra_program()
            .unwrap();
        let visited_pcs = visited_pcs_from_entrypoint(cairo_file, 0, &[]);

        let sierra_profiler = SierraProfiler::new(sierra_program.clone(), Some(1)).unwrap();

        let concrete_libfunc_weights =
            internal_extract_libfuncs_weight(&sierra_profiler, &visited_pcs);

        assert_eq!(concrete_libfunc_weights.len(), 30);
        assert_libfunc_frequency(
            &concrete_libfunc_weights,
            "const_as_immediate<Const<felt252, 7>>",
            1,
        );
        assert_libfunc_frequency(
            &concrete_libfunc_weights,
            "const_as_immediate<Const<felt252, 11>>",
            1,
        );
        assert_libfunc_frequency(
            &concrete_libfunc_weights,
            "const_as_immediate<Const<felt252, 13>>",
            1,
        );
        assert_libfunc_frequency(
            &concrete_libfunc_weights,
            "const_as_immediate<Const<felt252, 49>>",
            1,
        );
        assert_libfunc_frequency(
            &concrete_libfunc_weights,
            "const_as_immediate<Const<felt252, 17>>",
            1,
        );
        assert_libfunc_frequency(
            &concrete_libfunc_weights,
            "const_as_immediate<Const<felt252, 19>>",
            1,
        );
        assert_libfunc_frequency(
            &concrete_libfunc_weights,
            "const_as_immediate<Const<felt252, 23>>",
            1,
        );
        assert_libfunc_frequency(
            &concrete_libfunc_weights,
            "const_as_immediate<Const<felt252, 31>>",
            1,
        );
        assert_libfunc_frequency(&concrete_libfunc_weights, "felt252_add", 7);
        assert_libfunc_frequency(&concrete_libfunc_weights, "store_temp<felt252>", 8);
    }

    #[test]
    #[allow(clippy::too_many_lines)] // Allowed because this is a test.
    fn test_extract_libfuncs_dict() {
        // This is the original CAIRO code.
        // use core::dict::Felt252Dict;
        // #[starknet::interface]
        // trait HelloStarknetTrait<TContractState> {
        //     fn increase_balance(ref self: TContractState);
        // }
        // #[starknet::contract]
        // mod hello_starknet {
        //     #[storage]
        //     struct Storage {
        //     }
        //     #[abi(embed_v0)]
        //     impl HelloStarknetImpl of super::HelloStarknetTrait<ContractState> {
        //         fn increase_balance(ref self: ContractState) {
        //             let mut i: u8 = 0;
        //             loop {
        //                 if i >= 100 {
        //                     break;
        //                 }
        //                 let mut dict: Felt252Dict<u8> = Default::default();
        //                 dict.insert(i.into(), i);
        //                 i = i + 1;
        //             };
        //         }
        //     }
        // }

        let cairo_file = "/test_data/sierra_dict.cairo";
        let sierra_program = compile_cairo_contract(cairo_file, true)
            .extract_sierra_program()
            .unwrap();
        let visited_pcs = visited_pcs_from_entrypoint(cairo_file, 0, &[]);

        let sierra_profiler = SierraProfiler::new(sierra_program.clone(), Some(1)).unwrap();

        let concrete_libfunc_weights =
            internal_extract_libfuncs_weight(&sierra_profiler, &visited_pcs);

        assert_eq!(concrete_libfunc_weights.len(), 45);
        assert_libfunc_frequency(
            &concrete_libfunc_weights,
            "drop<SquashedFelt252Dict<u8>>",
            100,
        );
        assert_libfunc_frequency(
            &concrete_libfunc_weights,
            "felt252_dict_entry_finalize<u8>",
            100,
        );
        assert_libfunc_frequency(&concrete_libfunc_weights, "felt252_dict_entry_get<u8>", 100);
        assert_libfunc_frequency(&concrete_libfunc_weights, "felt252_dict_squash<u8>", 100);
        assert_libfunc_frequency(
            &concrete_libfunc_weights,
            "store_temp<Felt252Dict<u8>>",
            200,
        );
        assert_libfunc_frequency(
            &concrete_libfunc_weights,
            "store_temp<SquashedFelt252Dict<u8>>",
            100,
        );
        assert_libfunc_frequency(&concrete_libfunc_weights, "felt252_dict_new<u8>", 100);
    }

    #[test]
    fn test_extract_libfuncs_secp_contract() {
        let cairo_file = "/test_data/sierra_secp_contract.cairo";
        let sierra_program = compile_cairo_contract(cairo_file, true)
            .extract_sierra_program()
            .unwrap();
        // These visited_pcs were determined from execution of the contract using the
        // blockifier. The blockifier starts from pc 0.
        let visited_pcs: Vec<usize> = vec![
            0, 7, 9, 10, 12, 13, 15, 31, 33, 35, 36, 45, 47, 48, 50, 52, 54, 56, 58, 60, 61, 62,
            63, 64, 65, 66, 67, 69, 70, 71, 72, 74, 76, 78, 80, 82, 83, 84, 85, 86, 87, 88, 90, 92,
            93, 94, 96, 98, 99, 100,
        ];

        let sierra_profiler = SierraProfiler::new(sierra_program.clone(), None).unwrap();

        let concrete_libfunc_weights =
            internal_extract_libfuncs_weight(&sierra_profiler, &visited_pcs);

        assert_eq!(concrete_libfunc_weights.len(), 30);
        assert_libfunc_frequency(&concrete_libfunc_weights, "secp256r1_new_syscall", 1);
        assert_libfunc_frequency(&concrete_libfunc_weights, "secp256r1_mul_syscall", 1);
        assert_libfunc_frequency(&concrete_libfunc_weights, "drop<Secp256r1Point>", 1);
    }
}
