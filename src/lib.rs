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
mod snapshot;
mod stats;
mod pretty;

#[cfg(test)]
mod testing;

pub use self::config::Config;
pub use self::errors::{Error, ErrorKind};
pub use self::commands::{Pull, Push};
pub use self::stats::Stats;

