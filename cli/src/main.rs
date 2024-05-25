//! Re-execute Starknet transactions in a range of blocks.
//!
//! Iterates over specified blocks in the database and re-executes all
//! transactions within those blocks. This is only the CLI front-end. All the
//! logic is contained in the library `cairo-replay`.

#![warn(clippy::all, clippy::cargo, clippy::pedantic)]
#![allow(clippy::multiple_crate_versions)] // Due to duplicate dependencies in pathfinder

use std::path::PathBuf;

use anyhow::bail;
use cairo_replay::{
    connect_to_database,
    get_latest_block_number,
    run_replay,
    ReplayRange,
};
use clap::Parser;
use itertools::Itertools;

// The Cairo VM allocates felts on the stack, so during execution it's making
// a huge number of allocations. We get roughly two times better execution
// performance by using jemalloc (compared to the Linux glibc allocator).
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[derive(Clone, Parser, Debug)]
struct Args {
    #[arg(long)]
    /// The path of the Pathfinder database file.
    db_path: PathBuf,

    #[arg(long)]
    /// The starting block to replay transactions.
    start_block: u64,

    #[arg(long)]
    /// The final block (included) to stop replaying transactions. It is
    /// reduced if bigger than the biggest block in the database.
    end_block: u64,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .compact()
        .init();

    let args = Args::parse();

    let database_path = args.db_path;
    let start_block = args.start_block;
    let end_block = args.end_block;

    run(start_block, end_block, database_path)
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
) -> anyhow::Result<()> {
    if start_block > end_block {
        bail!(
            "Exiting because end_block must be greater or equal to \
             start_block."
        )
    }

    let storage = connect_to_database(database_path)?;

    let first_block: u64 = start_block;

    let latest_block: u64 = get_latest_block_number(&storage)?;

    let last_block: u64 = end_block.min(latest_block);

    if start_block > last_block {
        bail!(
            "Most recent block found in the databse is {}. Exiting because \
             less than start_block {}",
            last_block,
            start_block
        )
    }

    let replay_range = ReplayRange::new(first_block, last_block)?;

    tracing::info!(%first_block, %last_block, "Re-executing blocks");

    let start_time = std::time::Instant::now();
    let libfunc_stats = run_replay(&replay_range, storage)?;

    for (concrete_name, weight) in
        libfunc_stats.iter().sorted_by(|a, b| Ord::cmp(&a.1, &b.1))
    {
        tracing::info!("  libfunc {concrete_name}: {weight}");
    }

    let elapsed = start_time.elapsed();

    tracing::info!(?elapsed, "Finished");

    Ok(())
}
