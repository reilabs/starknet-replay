//! This module exports `ReplayStatistics` to a text file.

use std::fs;
use std::path::PathBuf;

use crate::error::ProfilerError;
use crate::ReplayStatistics;

/// This function writes a `ReplayStatistics` object in CSV format to a file.
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
    filename: &PathBuf,
    replay_statistics: &ReplayStatistics,
) -> Result<(), ProfilerError> {
    let output = replay_statistics.to_csv()?;
    fs::write(filename, output)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::Read;

    use indoc::indoc;

    use super::*;

    fn read_file(filename: &PathBuf) -> String {
        let mut file = File::open(filename).unwrap();
        let mut buffer = String::new();
        file.read_to_string(&mut buffer).unwrap();
        buffer
    }

    #[test]
    fn test_write_to_file() {
        let mut replay_statistics = ReplayStatistics::default();
        replay_statistics.update(&"u32_to_felt252".to_string(), 759);
        replay_statistics.update(&"const_as_immediate".to_string(), 264);

        let filename = "test_write_to_file.log";
        write_to_file(&filename.into(), &replay_statistics).unwrap();

        // Don't forget libfuncs are reported in ascending order of weight.
        let expected_string = indoc! {r"
            Function Name,Weight
            const_as_immediate,264
            u32_to_felt252,759
        "};
        assert_eq!(read_file(&filename.into()), expected_string);
    }
}
