#![warn(rust_2018_idioms)]
#![allow(unstable_name_collisions)]

mod bytes;
mod commands;
mod config;
mod errors;
mod hasher;
mod snapshot;

#[cfg(test)]
mod testing;

pub use self::config::Config;
pub use self::errors::{Error, ErrorKind};

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
