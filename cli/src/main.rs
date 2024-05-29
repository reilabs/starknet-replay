//! Re-execute Starknet transactions in a range of blocks.
//!
//! Iterates over specified blocks in the database and re-executes all
//! transactions within those blocks. This is only the CLI front-end. All the
//! logic is contained in the library `cairo-replay`.

#![warn(clippy::all, clippy::cargo, clippy::pedantic)]
#![allow(clippy::multiple_crate_versions)] // Due to duplicate dependencies in pathfinder

use std::path::PathBuf;
use std::process;

use anyhow::bail;
use cairo_replay::error::DatabaseError;
use cairo_replay::{
    connect_to_database,
    export_histogram,
    get_latest_block_number,
    run_replay,
    ReplayRange,
};
use clap::Parser;
use exitcode::{OK, SOFTWARE};
use itertools::Itertools;

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
    /// If the file exists already, it tries overwriting.
    #[arg(long)]
    svg_out: Option<PathBuf>,
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .compact()
        .init();

    let args = Args::parse();

    let database_path = args.db_path;
    let start_block = args.start_block;
    let end_block = args.end_block;
    let svg_path = args.svg_out;

    match run(start_block, end_block, database_path, svg_path) {
        Ok(()) => process::exit(OK),
        Err(e) => {
            eprintln!("Internal software error: {e}");
            process::exit(SOFTWARE);
        }
    }
}

/// Take the command line input arguments and call cairo-replay.
///
/// Sanitisation of the inputs is done in this function.
///
/// # Arguments
///
/// - `start_block`: First block to replay.
/// - `end_block`: Final block to replay.
/// - `database_path`: Path of the Pathfinder database.
/// - `svg_path`: Output path of the libfunc histogram SVG image.
///
/// # Errors
///
/// Returns [`Err`] if:
///
/// - `start_block` is greater than `end_block`.
/// - Not enough blocks in the database to cover the required range of blocks to
///   replay.
/// - Any error during execution of `cairo-replay`.
fn run(
    start_block: u64,
    end_block: u64,
    database_path: PathBuf,
    svg_path: Option<PathBuf>,
) -> anyhow::Result<()> {
    if start_block > end_block {
        bail!("Exiting because end_block must be greater or equal to start_block.")
    }

    let storage = connect_to_database(database_path)?;

    let first_block: u64 = start_block;

    let latest_block: u64 = get_latest_block_number(&storage)?;

    let last_block: u64 = end_block.min(latest_block);

    if start_block > last_block {
        return Err(DatabaseError::InsufficientBlocks {
            last_block,
            start_block,
        }
        .into());
    }

    let replay_range = ReplayRange::new(first_block, last_block)?;

    tracing::info!(%first_block, %last_block, "Re-executing blocks");
    let start_time = std::time::Instant::now();

    let libfunc_stats = run_replay(&replay_range, storage)?;

    let elapsed = start_time.elapsed();
    tracing::info!(?elapsed, "Finished");

    for (concrete_name, weight) in libfunc_stats
        .concrete_libfunc
        .iter()
        .sorted_by(|a, b| Ord::cmp(&a.1, &b.1))
    {
        tracing::info!("  libfunc {concrete_name}: {weight}");
    }

    match svg_path {
        Some(filename) => {
            let title = format!("Libfuncs usage from block {first_block} to block {last_block}");
            export_histogram(&filename, title.as_str(), &libfunc_stats)?;
            Ok(())
        }
        None => Ok(()),
    }
}
