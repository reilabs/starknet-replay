//! This file contains the structure of command line arguments supported by the
//! tool.

use std::path::PathBuf;

use clap::Parser;
use url::Url;

/// This is the struct of the command line arguments accepted by
/// `starknet-replay`.
#[derive(Clone, Parser, Debug)]
pub struct Args {
    /// The url of the RPC node.
    #[arg(long)]
    pub rpc_url: Url,

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

    /// The filename to output transaction traces from the replay.
    ///
    /// If `None`, output file is skipped.
    #[arg(long)]
    pub trace_out: Option<PathBuf>,

    /// Set to overwrite `svg_out`, `txt_out`, `trace_out` if they already
    /// exists.
    #[arg(long)]
    pub overwrite: bool,

    /// Set to perform serial replay of blocks.
    ///
    /// Slower, but forces the initial state of block `n+1` to be consistent
    /// with the final state of block `n`. This is not ensured with parallel
    /// replay because the final state may differ with the final state
    /// fetched from the chain.
    #[arg(long)]
    pub serial_replay: bool,
}
