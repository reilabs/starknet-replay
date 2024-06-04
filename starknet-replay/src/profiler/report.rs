//! This module exports `ReplayStatistics` to a text file.

use std::fs;
use std::path::Path;

use crate::error::ProfilerError;
use crate::ReplayStatistics;

/// This function writes a `ReplayStatistics` object to a file.
///
/// If the file exists already, it is overwritten.
///
/// # Arguments
///
/// - `filename`: the file to write.
/// - `replay_statistics`: the `ReplayStatistics` object.
///
/// # Errors
///
/// Returns [`Err`] if:
///
/// - `filename` can't be written to.
/// - The list of parent directories in `filename` don't exist.
pub fn write_to_file(
    filename: &Path,
    replay_statistics: &ReplayStatistics,
) -> Result<(), ProfilerError> {
    let content = replay_statistics.to_string();
    fs::write(filename, content)?;
    Ok(())
}
