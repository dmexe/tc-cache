use std::collections::HashMap;
use std::convert::TryFrom;
use std::fmt::{Debug, Display};
use std::path::PathBuf;

use url::Url;

mod s3;

pub use self::s3::S3;
use crate::Error;

#[derive(Debug)]
pub struct DownloadRequest {
    pub path: PathBuf,
    pub key: String,
}

#[derive(Debug, Default)]
pub struct UploadRequest {
    pub path: PathBuf,
    pub len: usize,
    pub key: String,
    pub tags: HashMap<String, String>,
}

pub trait Remote: Display + Debug {
    fn key(self, key: &str) -> Self;

    fn download(&self, req: DownloadRequest) -> Result<(), Error>;

    fn upload(&self, req: UploadRequest) -> Result<(), Error>;
}
