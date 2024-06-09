//! The module contains internal components required to perform transactions
//! replay. This is needed to make the tool node-agnostic.
//! Each component has implementations to convert between node structure and
//! `starknet-replay` equivalent.

pub use block_number::BlockNumber;

pub mod block_number;
pub mod storage;
