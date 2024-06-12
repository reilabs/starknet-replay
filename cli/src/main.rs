//! Re-execute Starknet transactions in a range of blocks.
//!
//! Iterates over specified blocks in the database and re-executes all
//! transactions within those blocks. This is only the CLI front-end. All the
//! logic is contained in the library [`starknet_replay`].

#![warn(clippy::all, clippy::cargo, clippy::pedantic)]
#![allow(clippy::multiple_crate_versions)] // Due to duplicate dependencies in pathfinder

use std::path::PathBuf;
use std::process;

use anyhow::bail;
use clap::Parser;
use exitcode::{OK, SOFTWARE};
use starknet_replay::histogram::export as export_histogram;
use starknet_replay::profiler::analysis::extract_libfuncs_weight;
use starknet_replay::profiler::report::write_to_file;
use starknet_replay::runner::replay_range::ReplayRange;
use starknet_replay::runner::run_replay;
use starknet_replay::storage::pathfinder::PathfinderStorage;

// The Cairo VM allocates felts on the stack, so during execution it's making
// a huge number of allocations. We get roughly two times better execution
// performance by using jemalloc (compared to the Linux glibc allocator).
// TODO: review in other operating systems. Issue #21
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[derive(Clone, Parser, Debug)]
struct Args {
    /// The path of the Pathfinder database file.
    #[arg(long)]
    db_path: PathBuf,

    /// The starting block to replay transactions.
    #[arg(long)]
    start_block: u64,

    /// The final block (included) to stop replaying transactions. It is
    /// reduced if bigger than the biggest block in the database.
    #[arg(long)]
    end_block: u64,

    /// The filename of the histogram SVG image.
    ///
    /// If `None`, histogram generation is skipped.
    #[arg(long)]
    svg_out: Option<PathBuf>,

    /// The filename to output the raw libfunc usage statistics.
    ///
    /// If `None`, output file is skipped.
    #[arg(long)]
    txt_out: Option<PathBuf>,

    /// Set to overwrite `svg_out` and/or `txt_out` if it already exists.
    #[arg(long)]
    overwrite: bool,
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .compact()
        .init();

    let args = Args::parse();

    match run(args) {
        Ok(()) => process::exit(OK),
        Err(e) => {
            eprintln!("Internal software error: {e}");
            process::exit(SOFTWARE);
        }
    }
}

/// Returns an error if the file exists already and can't be overwritten,
///
/// # Arguments
///
/// - `path`: The file to write.
/// - `overwrite`: If `true`, the file can be overwritten.
fn check_file(path: &Option<PathBuf>, overwrite: bool) -> anyhow::Result<()> {
    if let Some(filename) = path {
        if filename.exists() && !overwrite {
            let filename = filename.as_path().display();
            bail!(
                "The file {0:?} exists already. To ignore it, pass the flag --overwrite.",
                filename
            )
        }
    }
    Ok(())
}

/// Take the command line input arguments and call the replayer.
///
/// Sanitisation of the inputs is done in this function.
///
/// # Arguments
///
/// - `args`: The list of command line input arguments.
///
/// # Errors
///
/// Returns [`Err`] if:
///
/// - `start_block` is greater than `end_block`.
/// - Not enough blocks in the database to cover the required range of blocks to
///   replay.
/// - Any error during execution of the replayer..
fn run(args: Args) -> anyhow::Result<()> {
    let database_path = args.db_path;
    let start_block = args.start_block;
    let end_block = args.end_block;
    let svg_path = args.svg_out;
    let txt_out = args.txt_out;
    let overwrite = args.overwrite;

    check_file(&svg_path, overwrite)?;
    check_file(&txt_out, overwrite)?;

    let storage = PathfinderStorage::new(database_path)?;

    let replay_range = ReplayRange::new(start_block, end_block)?;

    tracing::info!(%start_block, %end_block, "Re-executing blocks");
    let start_time = std::time::Instant::now();

    let visited_pcs = run_replay(&replay_range, &storage.clone())?;

    let libfunc_stats = extract_libfuncs_weight(&visited_pcs, &storage)?;

    let elapsed = start_time.elapsed();
    tracing::info!(?elapsed, "Finished");

    if let Some(filename) = txt_out {
        write_to_file(&filename, &libfunc_stats)?;
    }

    if let Some(filename) = svg_path {
        let title =
            format!("Filtered libfuncs usage from block {start_block} to block {end_block}");
        let libfunc_stats = libfunc_stats.filter_most_frequent();
        export_histogram(&filename, title.as_str(), &libfunc_stats)?;
    }

    Ok(())
}
