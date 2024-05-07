use std::num::NonZeroU32;
use std::path::PathBuf;

use anyhow::Context;
use cairo_replay::run_replay;
use clap::Parser;
use pathfinder_storage::{BlockId, JournalMode, Storage};

use crate::utils::get_chain_id;

mod utils;

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

/// Re-execute transactions in a range of blocks.
///
/// Iterates over specified blocks in the database and re-executes all
/// transactions within those blocks
///
/// Usage:
/// `cargo run --release -- --db-path <PATHFINDER_DB> --start-block <BLOCK_NUM>
/// --end-block <BLOCK_NUM>`
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
        .create_pool(NonZeroU32::new(n_cpus as u32 * 2).unwrap())?;
    let mut db = storage
        .connection()
        .context("Opening database connection")?;

    let first_block: u64 = args.start_block;

    let (latest_block, chain_id) = {
        let tx = db.transaction().unwrap();
        let (latest_block, _) = tx.block_id(BlockId::Latest)?.unwrap();
        let latest_block = latest_block.get();
        let chain_id = get_chain_id(&tx).unwrap();
        (latest_block, chain_id)
    };

    let last_block = args.end_block.min(latest_block);

    tracing::info!(%first_block, %last_block, "Re-executing blocks");

    let start_time = std::time::Instant::now();
    let num_transactions: usize = run_replay(first_block, last_block, database_path, chain_id)?;

    let elapsed = start_time.elapsed();

    tracing::info!(%num_transactions, ?elapsed, "Finished");

    Ok(())
}
