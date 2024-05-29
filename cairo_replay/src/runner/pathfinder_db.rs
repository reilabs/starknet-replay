//! This module is an interface between the Pathfinder database API and
//! cairo-replay.

use std::num::NonZeroU32;
use std::path::PathBuf;

use anyhow::Context;
use pathfinder_storage::{BlockId, JournalMode, Storage};
use rayon::current_num_threads;

use crate::error::DatabaseError;

/// Connects to the Pathfinder database
///
/// The connection to the Pathfinder database is established with the
/// construction of the Storage object.
///
/// The number of parallel connections is set to be twice the number of threads
/// in the CPU: this is to ensure spare capacity. In case of panics, the default
/// number of connections is set to 1.
///
/// # Arguments
///
/// - `database_path`: Path of the Pathfinder database file.
///
/// # Errors
///
/// Returns [`Err`] if this function is called more than once in the
/// application.
pub fn connect_to_database(database_path: PathBuf) -> Result<Storage, DatabaseError> {
    let n_cpus = current_num_threads();
    let n_parallel_connections: u32 = n_cpus.checked_mul(2).unwrap_or(1).try_into().unwrap_or(1);
    let Some(capacity) = NonZeroU32::new(n_parallel_connections) else {
        unreachable!("n_parallel_connections should never be less than 1.")
    };

    let store_manager = Storage::migrate(database_path, JournalMode::WAL, 1)
        .map_err(DatabaseError::ConnectToDatabase)?;
    let pool = store_manager
        .create_pool(capacity)
        .map_err(DatabaseError::ConnectToDatabase)?;
    Ok(pool)
}

/// Returns the latest (most recent) block number in the database
///
/// If no block is found in the database, it returns 0.
///
/// # Arguments
///
/// - `storage`: The `Storage` object of the Pathfinder database connection.
///
/// # Errors
///
/// Returns [`Err`] if the low level API with the database returns an error.
pub fn get_latest_block_number(storage: &Storage) -> Result<u64, DatabaseError> {
    let mut db = storage
        .connection()
        .context("Opening database connection")
        .map_err(DatabaseError::GetLatestBlockNumber)?;
    let tx = db
        .transaction()
        .map_err(DatabaseError::GetLatestBlockNumber)?;
    let Some((latest_block, _)) = tx
        .block_id(BlockId::Latest)
        .map_err(DatabaseError::GetLatestBlockNumber)?
    else {
        drop(tx);
        drop(db);
        return Ok(0);
    };
    drop(tx);
    drop(db);
    Ok(latest_block.get())
}
