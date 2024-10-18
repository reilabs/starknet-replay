//! The module [`crate::profiler`] contains the code to process a sequence of
//! program counters and return the object
//! [`crate::profiler::replay_statistics::ReplayStatistics`] which contains call
//! frequency of libfuncs.

#![allow(clippy::module_name_repetitions)] // Added because of `SierraProfiler`

use std::collections::HashMap;
use std::fmt::{self, Display, Formatter};

use cairo_lang_casm::assembler::InstructionRepr;
use cairo_lang_casm::instructions::Instruction;
use cairo_lang_runner::RunnerError as CairoError;
use cairo_lang_sierra::program::{GenStatement, Program, Statement, StatementIdx};
use cairo_lang_sierra_to_casm::compiler::{compile, CairoProgram, SierraToCasmConfig};
use cairo_lang_sierra_to_casm::metadata::{
    calc_metadata,
    calc_metadata_ap_change_only,
    Metadata,
    MetadataComputationConfig,
    MetadataError,
};
use tracing::trace;

use crate::error::ProfilerError;

pub mod analysis;
pub mod replace_ids;
pub mod replay_statistics;
pub mod report;

/// This structure contains the mapping between the Sierra statement, CASM
/// instruction and memory opcode.
#[derive(Debug, Eq, PartialEq)]
pub struct CompiledStatement {
    /// The Sierra statement
    sierra_statement: Statement,

    /// The CASM instruction
    casm_instruction: Instruction,

    /// The memory data
    memory: InstructionRepr,

    /// The program counter at the beginning of the memory content (starting
    /// from 1)
    pc: usize,

    /// The statement index (starting from 1)
    statement_idx: usize,

    /// The CASM instruction index (starting from 1)
    instruction_idx: usize,
}
impl Display for CompiledStatement {
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

/// The core of the libfunc profiler which extracts libfunc frequencies from the
/// list of visited program counters.
pub struct SierraProfiler {
    /// The sierra program.
    pub sierra_program: Program,

    /// The casm program matching the Sierra code.
    pub casm_program: CairoProgram,

    /// The vector containing the whole program with correspondance between
    /// Sierra, CASM and memory.
    pub commands: Vec<CompiledStatement>,
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

        let mut commands: Vec<CompiledStatement> = Vec::new();
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
                    let command = CompiledStatement {
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

    /// Returns the map between the Sierra statement id and the frequency of
    /// usage.
    ///
    /// # Arguments
    ///
    /// - `pcs`: The sequence of program counters to analyse.
    #[must_use]
    pub fn collect_profiling_info(&self, pcs: &[usize]) -> HashMap<StatementIdx, usize> {
        let mut sierra_statement_weights = HashMap::default();
        for pc in pcs {
            let statements: Vec<&CompiledStatement> =
                self.commands.iter().filter(|c| c.pc == *pc).collect();
            for statement in statements {
                let statement_idx = StatementIdx(statement.statement_idx);
                *sierra_statement_weights.entry(statement_idx).or_insert(0) += 1;
            }
        }

        sierra_statement_weights
    }

    /// Translates the given Sierra statement index into the actual statement.
    ///
    /// # Arguments
    ///
    /// - `statement_idx`: The Sierra statement id.
    fn statement_idx_to_gen_statement(
        &self,
        statement_idx: StatementIdx,
    ) -> Option<GenStatement<StatementIdx>> {
        self.sierra_program.statements.get(statement_idx.0).cloned()
    }

    /// Returns the map between the concrete libfunc and the frequency of
    /// usage.
    ///
    /// # Arguments
    ///
    /// - `statements`: The map with the frequency of Sierra statements.
    #[must_use]
    pub fn unpack_profiling_info(
        &self,
        statements: &HashMap<StatementIdx, usize>,
    ) -> HashMap<String, usize> {
        let mut libfunc_weights = HashMap::<String, usize>::default();
        for (statement_idx, frequency) in statements {
            if let Some(GenStatement::Invocation(invocation)) =
                self.statement_idx_to_gen_statement(*statement_idx)
            {
                let name = invocation.libfunc_id.to_string();
                *(libfunc_weights.entry(name).or_insert(0)) += frequency;
            }
        }
        libfunc_weights
    }
}
