//! The module [`crate::profiler`] contains the code to process a sequence of
//! program counters and return the object
//! [`crate::profiler::replay_statistics::ReplayStatistics`] which contains call
//! frequency of libfuncs.

#![allow(clippy::module_name_repetitions)] // Added because of `SierraProfiler`

use cairo_lang_runner::profiling::{user_function_idx_by_sierra_statement_idx, ProfilingInfo};
use cairo_lang_runner::{ProfilingInfoCollectionConfig, RunnerError as CairoError};
use cairo_lang_sierra::extensions::core::{CoreConcreteLibfunc, CoreLibfunc, CoreType};
use cairo_lang_sierra::program::{GenStatement, Program, StatementIdx};
use cairo_lang_sierra::program_registry::ProgramRegistry;
use cairo_lang_sierra_to_casm::compiler::{compile, CairoProgram, SierraToCasmConfig};
use cairo_lang_sierra_to_casm::metadata::{
    calc_metadata,
    calc_metadata_ap_change_only,
    Metadata,
    MetadataComputationConfig,
    MetadataError,
};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use itertools::chain;

use crate::error::ProfilerError;

pub mod analysis;
pub mod replace_ids;
pub mod replay_statistics;
pub mod report;

/// The default maximum depth for a stack trace.
///
/// This number has been determined to be large enough to handle the contracts
/// on Starknet via empirical testing.
const MAX_STACK_TRACE_DEPTH_DEFAULT: usize = 10000;

/// Creates the metadata required for a lowering a Sierra program to CASM.
///
/// This function is copied from crate [`cairo_lang_runner`] because it
/// isn't public.
///
/// # Arguments
///
/// - `sierra_program`: The sierra program.
/// - `metadata_config`: The configuration options. If not provided,
///   `create_metadata` will skip gas usage calculations.
///
/// # Errors
///
/// Returns [`Err`] if:
///
/// - Call to [`calc_metadata`] fails
/// - Call to [`calc_metadata_ap_change_only`] fails
// TODO: Change `cairo` crate and make `create_metadata` public. Issue #23.
fn create_metadata(
    sierra_program: &Program,
    metadata_config: Option<MetadataComputationConfig>,
) -> Result<Metadata, ProfilerError> {
    let metadata = if let Some(metadata_config) = metadata_config {
        calc_metadata(sierra_program, metadata_config)
    } else {
        calc_metadata_ap_change_only(sierra_program)
    }
    .map_err(|err| match err {
        MetadataError::ApChangeError(err) => CairoError::ApChangeError(err),
        MetadataError::CostError(_) => CairoError::FailedGasCalculation,
    })?;
    Ok(metadata)
}

/// Extracts profiling data from the list of visited program counters.
///
/// This is a slimmed down version of [`cairo_lang_runner::SierraCasmRunner`]
/// adapted for use in Starknet contracts instead of Cairo programs. It is
/// needed to setup the profiler during transaction replay. There is no call to
/// [`cairo-vm`] because this slimmed down version takes the list of visited
/// program counters as input to
/// [`SierraProfiler#method.collect_profiling_info`].
pub struct SierraProfiler {
    /// The sierra program.
    sierra_program: Program,

    /// Program registry for the Sierra program.
    sierra_program_registry: ProgramRegistry<CoreType, CoreLibfunc>,

    /// The casm program matching the Sierra code.
    casm_program: CairoProgram,

    /// Whether to run the profiler when running using this runner.
    pub run_profiler: Option<ProfilingInfoCollectionConfig>,
}
impl SierraProfiler {
    /// Generates a new [`SierraProfiler`] object.
    ///
    /// # Arguments
    ///
    /// - `sierra_program`: The sierra program considered in the runner.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if:
    ///
    /// - The call to `create_metadata` fails
    /// - The generation of `[sierra_program_registry`] fails
    pub fn new(sierra_program: Program) -> Result<Self, ProfilerError> {
        // `run_profiler` and `metadata_config` are set as per default values
        // preventing the user from choosing `None` as in the original
        // `SierraCasmRunner`. This is to ensure the profiler is always run with
        // the same configuration.
        let run_profiler = Some(ProfilingInfoCollectionConfig::default());
        let metadata_config = Some(MetadataComputationConfig::default());
        let gas_usage_check = metadata_config.is_some();
        let metadata = create_metadata(&sierra_program, metadata_config)?;
        let sierra_program_registry =
            ProgramRegistry::<CoreType, CoreLibfunc>::new(&sierra_program)?;
        let casm_program = compile(
            &sierra_program,
            &metadata,
            SierraToCasmConfig {
                gas_usage_check,
                max_bytecode_size: usize::MAX,
            },
        )?;

        Ok(Self {
            sierra_program,
            sierra_program_registry,
            casm_program,
            run_profiler,
        })
    }

    /// Returns the index of the Sierra statement at `pc`.
    // TODO: To be refactored. Issue #5.
    fn sierra_statement_index_by_pc(&self, pc: usize) -> StatementIdx {
        // The `-1` here can't cause an underflow as the first statement is
        // always at offset 0, so it is always on the left side of the
        // partition, and thus the partition index is >0.
        StatementIdx(
            self.casm_program
                .debug_info
                .sierra_statement_info
                .partition_point(|x| x.code_offset <= pc)
                - 1,
        )
    }

    /// Collects profiling info of the current run using the trace.
    ///
    /// This function has been copied from
    /// [`cairo_lang_runner::SierraCasmRunner#method.collect_profiling_info`]
    /// but it was written for Cairo programs. It needs to be adapted for
    /// use with Starknet contracts.
    ///
    /// In particular, the variable `end_of_program_reached` doesn't
    /// seem to be valid for Starknet contracts.
    ///
    /// # Errors
    ///
    /// Returns [`Err`] if:
    ///
    /// - `pcs` is empty.
    /// - The CASM program has no Sierra statements.
    /// - Sierra statement not found.
    // TODO: To be refactored. Issue #5.
    pub fn collect_profiling_info(&self, pcs: &[usize]) -> Result<ProfilingInfo, ProfilerError> {
        let sierra_len = self.casm_program.debug_info.sierra_statement_info.len();
        let bytecode_len = self
            .casm_program
            .debug_info
            .sierra_statement_info
            .last()
            .ok_or(ProfilerError::EmptyStatementList)?
            .code_offset;
        // The CASM program starts with a header of instructions to wrap the
        // real program. `real_pc_0` is the PC in the trace that points
        // to the same CASM instruction which is in the real PC=0 in the
        // original CASM program. That is, all trace's PCs need to be
        // subtracted by `real_pc_0` to get the real PC they point to in
        // the original CASM program.
        // This is the same as the PC of the last trace entry plus 1, as the
        // header is built to have a `ret` last instruction, which must
        // be the last in the trace of any execution. The first
        // instruction after that is the first instruction in the
        // original CASM program.
        let real_pc_0 = pcs.last().ok_or(ProfilerError::EmptyProgramCounterList)? + 1;

        // The function stack trace of the current function, excluding the
        // current function (that is, the stack of the caller).
        // Represented as a vector of indices of the functions in the
        // stack (indices of the functions according to the list in the
        // sierra program). Limited to depth `max_stack_trace_depth`.
        // Note `function_stack_depth` tracks the real depth, even if >=
        // `max_stack_trace_depth`.
        let mut function_stack = Vec::new();
        // Tracks the depth of the function stack, without limit. This is
        // usually equal to `function_stack.len()`, but if the actual
        // stack is deeper than `max_stack_trace_depth`, this remains
        // reliable while `function_stack` does not.
        let mut function_stack_depth = 0;
        let mut cur_weight = 0;
        // The key is a function stack trace (see `function_stack`, but
        // including the current function).
        // The value is the weight of the stack trace so far, not including the
        // pending weight being tracked at the time.
        let mut stack_trace_weights = UnorderedHashMap::default();
        // let mut _end_of_program_reached = false;
        // The total weight of each Sierra statement.
        // Note the header and footer (CASM instructions added for running the
        // program by the runner). The header is not counted, and the
        // footer is, but then the relevant entry is removed.
        let mut sierra_statement_weights = UnorderedHashMap::default();
        for step in pcs {
            // Skip the header.
            if *step < real_pc_0 {
                continue;
            }
            let real_pc = step - real_pc_0;
            // Skip the footer.
            if real_pc == bytecode_len {
                continue;
            }

            cur_weight += 1;

            // TODO(yuval): Maintain a map of pc to sierra statement index (only
            // for PCs we saw), to save lookups.
            let sierra_statement_idx = self.sierra_statement_index_by_pc(real_pc);
            let user_function_idx = user_function_idx_by_sierra_statement_idx(
                &self.sierra_program,
                sierra_statement_idx,
            );

            *sierra_statement_weights
                .entry(sierra_statement_idx)
                .or_insert(0) += 1;

            let Some(gen_statement) = self.sierra_program.statements.get(sierra_statement_idx.0)
            else {
                return Err(ProfilerError::SierraStatementNotFound(
                    sierra_statement_idx.0,
                ));
            };

            match gen_statement {
                GenStatement::Invocation(invocation) => {
                    let libfunc_found = self
                        .sierra_program_registry
                        .get_libfunc(&invocation.libfunc_id);
                    if matches!(libfunc_found, Ok(CoreConcreteLibfunc::FunctionCall(_))) {
                        // Push to the stack.
                        if function_stack_depth < MAX_STACK_TRACE_DEPTH_DEFAULT {
                            function_stack.push((user_function_idx, cur_weight));
                            cur_weight = 0;
                        } else {
                            tracing::info!("Exceeding depth");
                        }
                        function_stack_depth += 1;
                    }
                }
                GenStatement::Return(_) => {
                    // Pop from the stack.
                    if function_stack_depth <= MAX_STACK_TRACE_DEPTH_DEFAULT {
                        // The current stack trace, including the current
                        // function.
                        let cur_stack: Vec<_> =
                            chain!(function_stack.iter().map(|f| f.0), [user_function_idx])
                                .collect();
                        *stack_trace_weights.entry(cur_stack).or_insert(0) += cur_weight;

                        let Some(popped) = function_stack.pop() else {
                            // End of the program. Not valid for Starknet
                            // contracts.
                            continue;
                        };
                        cur_weight += popped.1;
                    } else {
                        tracing::info!("Exceeding depth");
                    }
                    function_stack_depth -= 1;
                }
            }
        }

        // Remove the footer.
        sierra_statement_weights.remove(&StatementIdx(sierra_len));

        Ok(ProfilingInfo {
            sierra_statement_weights,
            stack_trace_weights,
        })
    }
}
