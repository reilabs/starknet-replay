#![warn(clippy::all, clippy::cargo, clippy::pedantic)]
#![allow(clippy::multiple_crate_versions)]

//! Re-execute transactions in a range of blocks.
//!
//! Iterates over specified blocks in the database and re-executes all
//! transactions within those blocks
//!
//! Usage:
//! `cargo run --release -- --db-path <PATHFINDER_DB> --start-block <BLOCK_NUM>
//! --end-block <BLOCK_NUM>`

use std::num::NonZeroU32;
use std::path::PathBuf;

use anyhow::Context;
use cairo_replay::run_replay;
use clap::Parser;
use pathfinder_storage::{BlockId, JournalMode, Storage};

// The Cairo VM allocates felts on the stack, so during execution it's making
// a huge number of allocations. We get roughly two times better execution
// performance by using jemalloc (compared to the Linux glibc allocator).
#[global_allocator]
static GLOBAL: jemallocator::Jemalloc = jemallocator::Jemalloc;

#[derive(Parser, Debug)]
struct Args {
    #[arg(long)]
    db_path: PathBuf,
    #[arg(long)]
    start_block: u64,
    #[arg(long)]
    end_block: u64,
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .compact()
        .init();

    let args = Args::parse();

    let n_cpus = rayon::current_num_threads();

    let database_path = args.db_path;
    let storage = Storage::migrate(database_path.clone(), JournalMode::WAL, 1)?
        .create_pool(
            NonZeroU32::new(n_cpus.checked_mul(2).unwrap().try_into().unwrap())
                .unwrap(),
        )?;
    let mut db = storage
        .connection()
        .context("Opening database connection")?;

    let first_block: u64 = args.start_block;

    let latest_block = {
        let tx = db.transaction().unwrap();
        let (latest_block, _) = tx.block_id(BlockId::Latest)?.unwrap();
        drop(tx);
        drop(db);
        latest_block.get()
    };

    let last_block = args.end_block.min(latest_block);

    tracing::info!(%first_block, %last_block, "Re-executing blocks");

    let start_time = std::time::Instant::now();
    run_replay(first_block, last_block, storage)?;

    let elapsed = start_time.elapsed();

    tracing::info!(?elapsed, "Finished");

    Ok(())
}
