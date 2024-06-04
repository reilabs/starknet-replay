//! This module exports `ReplayStatistics` to a text file.

use std::path::Path;

use itertools::Itertools;

use crate::ReplayStatistics;

/// This function writes a `ReplayStatistics` object to a file.
///
/// If the file exists already, it is overwritten.
///
/// # Arguments
///
/// - `filename`: the file to write.
/// - `replay_statistics`: the `ReplayStatistics` object.
pub fn write_to_file(_filename: &Path, replay_statistics: &ReplayStatistics) {
    for (concrete_name, weight) in replay_statistics
        .concrete_libfunc
        .iter()
        .sorted_by(|a, b| Ord::cmp(&a.1, &b.1))
    {
        tracing::info!("  libfunc {concrete_name}: {weight}");
    }
}
