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
pub mod pretty;
mod remote;
mod services;
mod snapshot;
mod stats;

#[cfg(test)]
mod testing;

pub use self::commands::{Pull, Push};
pub use self::config::Config;
pub use self::errors::{Error, ErrorKind};
pub use self::remote::Remote;
pub use self::services::{Service, TeamCity};
pub use self::stats::Stats;
