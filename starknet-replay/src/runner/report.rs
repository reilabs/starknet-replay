use std::fs::OpenOptions;
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use pathfinder_executor::types::TransactionSimulation;

use crate::error::RunnerError;

/// Writes transaction traces to JSON file.
///
/// Transaction traces are appended to the file.
///
/// # Arguments
///
/// - `filename`: The file to output traces.
/// - `traces`: The list of traces to append.
///
/// # Errors
///
/// Returns [`Err`] if there is any error writing to `filename`.
pub fn write_to_file(
    filename: &PathBuf,
    traces: &Vec<TransactionSimulation>,
) -> Result<(), RunnerError> {
    let output_file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(filename)?;
    let mut f = BufWriter::new(output_file);
    for trace in traces {
        let output = serde_json::to_string(&trace.trace)?;
        f.write_all(output.as_bytes())?;
    }
    Ok(())
}
