use std::fmt::Debug;
use std::path::PathBuf;

mod s3;

pub use self::s3::S3;
use crate::Error;

#[derive(Debug)]
pub struct DownloadRequest {
    pub path: PathBuf,
    pub key: String,
}

#[derive(Debug)]
pub struct UploadRequest {
    pub path: PathBuf,
    pub len: usize,
    pub key: String,
}

pub trait Backend: Debug {
    fn download(&self, req: DownloadRequest) -> Result<usize, Error>;
    fn upload(&self, req: UploadRequest) -> Result<usize, Error>;
}
