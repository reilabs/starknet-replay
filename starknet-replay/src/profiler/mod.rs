//! The module [`crate::profiler`] contains the code to process a sequence of
//! program counters and return the object
//! [`crate::profiler::replay_statistics::ReplayStatistics`] which contains call
//! frequency of libfuncs.

#![allow(clippy::module_name_repetitions)] // Added because of `SierraProfiler`

use std::fmt::{self, Display, Formatter};

use cairo_lang_casm::assembler::InstructionRepr;
use cairo_lang_casm::instructions::Instruction;
use cairo_lang_runner::profiling::ProfilingInfo;
use cairo_lang_runner::RunnerError as CairoError;
use cairo_lang_sierra::program::{Program, Statement, StatementIdx};
use cairo_lang_sierra_to_casm::compiler::{compile, CairoProgram, SierraToCasmConfig};
use cairo_lang_sierra_to_casm::metadata::{
    calc_metadata,
    calc_metadata_ap_change_only,
    Metadata,
    MetadataComputationConfig,
    MetadataError,
};
use cairo_lang_utils::unordered_hash_map::UnorderedHashMap;
use tracing::trace;

use crate::error::ProfilerError;

pub mod analysis;
pub mod replace_ids;
pub mod replay_statistics;
pub mod report;

#[derive(Debug, Eq, PartialEq)]
pub struct ProgramProfiler {
    sierra_statement: Statement,
    casm_instruction: Instruction,
    memory: InstructionRepr,
    pc: usize,
    statement_idx: usize,
    instruction_idx: usize,
}
impl Display for ProgramProfiler {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(
            f,
            "Sierra {} | CASM {} | Memory {:?} | PC {} | Statement Idx {} | Instruction Idx {}",
            self.sierra_statement,
            self.casm_instruction.to_string().replace('\n', " "),
            self.memory.encode(),
            self.pc,
            self.statement_idx,
            self.instruction_idx
        )
    }
}

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
    pub sierra_program: Program,

    /// The casm program matching the Sierra code.
    pub casm_program: CairoProgram,

    pub commands: Vec<ProgramProfiler>,
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
        // `metadata_config` is set as per default values preventing the user from
        // choosing `None` as in the original `SierraCasmRunner`. This is to
        // ensure the profiler is always run with the same configuration.
        let metadata_config = Some(MetadataComputationConfig::default());
        let gas_usage_check = metadata_config.is_some();
        let metadata = create_metadata(&sierra_program, metadata_config)?;
        let casm_program = compile(
            &sierra_program,
            &metadata,
            SierraToCasmConfig {
                gas_usage_check,
                max_bytecode_size: usize::MAX,
            },
        )?;

        let mut commands: Vec<ProgramProfiler> = Vec::new();
        let mut last_pc: usize = 1;

        casm_program
            .instructions
            .iter()
            .enumerate()
            .for_each(|(instruction_idx, instruction)| {
                for (statement_idx, _) in casm_program
                    .debug_info
                    .sierra_statement_info
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| s.instruction_idx == instruction_idx)
                {
                    let sierra_statement = &sierra_program.statements[statement_idx];
                    let casm_instruction = instruction;
                    let command = ProgramProfiler {
                        sierra_statement: sierra_statement.clone(),
                        casm_instruction: casm_instruction.clone(),
                        memory: casm_instruction.assemble(),
                        pc: last_pc,
                        statement_idx,
                        instruction_idx,
                    };
                    trace!("{}", command);
                    commands.push(command);
                }
                last_pc += instruction.assemble().encode().len();
            });

        Ok(Self {
            sierra_program,
            casm_program,
            commands,
        })
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
    pub fn collect_profiling_info(&self, pcs: &[usize]) -> Result<ProfilingInfo, ProfilerError> {
        let stack_trace_weights = UnorderedHashMap::default();
        let mut sierra_statement_weights = UnorderedHashMap::default();
        for pc in pcs {
            let statements: Vec<&ProgramProfiler> =
                self.commands.iter().filter(|c| c.pc == *pc).collect();
            for statement in statements {
                let statement_idx = StatementIdx(statement.statement_idx);
                *sierra_statement_weights.entry(statement_idx).or_insert(0) += 1;
            }
        }

        Ok(ProfilingInfo {
            sierra_statement_weights,
            stack_trace_weights,
        })
    }
}
