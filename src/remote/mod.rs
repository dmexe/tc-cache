use std::path::Path;

use crate::Error;

mod s3;

pub trait Remote {
    fn download<P>(key: String, path: P) -> Result<(), Error>
    where
        P: AsRef<Path>;

    fn upload<P>(key: String, path: P) -> Result<(), Error>
    where
        P: AsRef<Path>;
}
