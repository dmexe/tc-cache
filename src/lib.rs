#![feature(try_from)]
#![feature(integer_atomics)]
#![feature(duration_as_u128)]
#![warn(rust_2018_idioms)]
#![allow(unstable_name_collisions)]

mod bytes;
mod commands;
mod config;
mod errors;
mod hashing;
mod mmap;
mod pretty;
mod services;
mod snapshot;
mod stats;
mod storage;

#[cfg(test)]
mod testing;

pub use self::commands::{Pull, Push};
pub use self::config::Config;
pub use self::errors::{Error, ErrorKind};
pub use self::services::{Service, TeamCity};
pub use self::stats::Stats;
pub use self::storage::Storage;
