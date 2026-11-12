//! The accepted pop file, durable storage and its writer lease.
mod error;
pub mod file;
pub mod images;
pub mod media;
pub mod registry;
pub mod store;
use pop_contracts::{Error, encode, err, hex};
use pop_core::{lock, random, random_hex};
type Result<T> = std::result::Result<T, Error>;
