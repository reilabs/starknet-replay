//! Re-execute Starknet transactions in a range of blocks.
//!
//! Iterates over specified blocks in the database and re-executes all
//! transactions within those blocks. This is only the CLI front-end. All the
//! logic is contained in the library [`starknet_replay`].

#![warn(
    clippy::all,
    clippy::cargo,
    clippy::pedantic,
    clippy::missing_docs_in_private_items,
    clippy::unwrap_used
)]
#![allow(clippy::multiple_crate_versions)] // Due to conflicts between dependencies of `starknet-crypto` and other crates.

use std::path::PathBuf;
use std::{fs, process};

use anyhow::bail;
use clap::Parser;
use exitcode::{OK, SOFTWARE};
use starknet_replay::histogram::export as export_histogram;
use starknet_replay::profiler::analysis::extract_libfuncs_weight;
use starknet_replay::profiler::report::write_to_file;
use starknet_replay::runner::replay_range::ReplayRange;
use starknet_replay::runner::run_replay;
use starknet_replay::storage::rpc::RpcStorage;

use crate::args::Args;

mod args;

// The Cairo VM allocates felts on the stack, so during execution it's making
// a huge number of allocations. We get roughly two times better execution
// performance by using jemalloc (compared to the Linux glibc allocator).
/// Custom allocator for efficiency, `msvc` is excluded because the library
/// can't be compiled natively on Windows machines.
#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

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

/// Returns an error if the file exists already and can't be overwritten.
///
/// If the file exists and it can be overwritten, it is deleted.
///
/// # Arguments
///
/// - `path`: The file to write.
/// - `overwrite`: If `true`, the file can be overwritten.
fn check_file(path: &Option<PathBuf>, overwrite: bool) -> anyhow::Result<()> {
    if let Some(filename) = path {
        if filename.exists() {
            if !overwrite {
                let filename = filename.as_path().display();
                bail!(
                    "The file {0:?} exists already. To ignore it, pass the flag --overwrite.",
                    filename
                )
            }
            fs::remove_file(filename)?;
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
    let rpc_url = args.rpc_url;
    let start_block = args.start_block;
    let end_block = args.end_block;
    let svg_path = args.svg_out;
    let txt_out = args.txt_out;
    let trace_out = args.trace_out;
    let overwrite = args.overwrite;

    check_file(&svg_path, overwrite)?;
    check_file(&txt_out, overwrite)?;
    check_file(&trace_out, overwrite)?;

    let storage = RpcStorage::new(rpc_url)?;

    let replay_range = ReplayRange::new(start_block, end_block)?;

    tracing::info!(%start_block, %end_block, "Re-executing blocks");
    let start_time = std::time::Instant::now();

    let visited_pcs = run_replay(&replay_range, &trace_out, &storage)?;

    let elapsed = start_time.elapsed();
    tracing::info!(?elapsed, "Finished");

    if txt_out.is_some() || svg_path.is_some() {
        let libfunc_stats = extract_libfuncs_weight(&visited_pcs, &storage)?;

        if let Some(filename) = txt_out {
            write_to_file(&filename, &libfunc_stats)?;
        }

        if let Some(filename) = svg_path {
            let title =
                format!("Filtered libfuncs usage from block {start_block} to block {end_block}");
            let libfunc_stats = libfunc_stats.filter_most_frequent();
            export_histogram(&filename, title.as_str(), &libfunc_stats)?;
        }
    }

    Ok(())
}
