use std::path::PathBuf;

use clap::Parser;

#[derive(Clone, Parser, Debug)]
pub struct Args {
    /// The path of the Pathfinder database file.
    #[arg(long)]
    pub db_path: PathBuf,

    /// The starting block to replay transactions.
    #[arg(long)]
    pub start_block: u64,

    /// The final block (included) to stop replaying transactions. It is
    /// reduced if bigger than the biggest block in the database.
    #[arg(long)]
    pub end_block: u64,

    /// The filename of the histogram SVG image.
    ///
    /// If `None`, histogram generation is skipped.
    #[arg(long)]
    pub svg_out: Option<PathBuf>,

    /// The filename to output the raw libfunc usage statistics.
    ///
    /// If `None`, output file is skipped.
    #[arg(long)]
    pub txt_out: Option<PathBuf>,

    /// The filename to output
    /// [`starknet_replay::transaction_trace::TransactionTrace`] from the
    /// replay.
    ///
    /// If `None`, output file is skipped.
    #[arg(long)]
    pub trace_out: Option<PathBuf>,

    /// Set to overwrite `svg_out` and/or `txt_out` if it already exists.
    #[arg(long)]
    pub overwrite: bool,
}
