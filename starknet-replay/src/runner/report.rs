use std::fs::OpenOptions;
use std::io::BufWriter;
use std::path::PathBuf;

use super::replay_class_hash::TransactionOutput;
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
    traces: &Vec<TransactionOutput>,
) -> Result<(), RunnerError> {
    let output_file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(filename)?;
    let mut _f = BufWriter::new(output_file);
    for (_trace, _) in traces {
        //let output = serde_json::to_string(&trace)?;
        //f.write_all(output.as_bytes())?;
    }
    Ok(())
}
