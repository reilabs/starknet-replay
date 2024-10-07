//! This module contains all the code related to the reading of the blokchcian
//! state through the RPC protocol.

#![allow(clippy::module_name_repetitions)]

pub mod permanent_state;
pub mod receipt;
pub mod replay_state_reader;
pub mod rpc_client;
pub mod transaction;
