use std::collections::HashMap;
use std::fmt::Display;
use std::fs::File;
use std::path::{Path, PathBuf};

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

pub trait Remote: Display {
    fn download(&self, req: DownloadRequest) -> Result<(), Error>;

    fn upload(&self, req: UploadRequest) -> Result<(), Error>;

    fn into_box(self) -> Box<dyn Remote>;
}

fn from<S>(uri: S) -> Result<Box<dyn Remote>, Error>
where
    S: AsRef<str>,
{
    let uri = Url::parse(uri.as_ref()).map_err(Error::remote)?;
    if uri.scheme() == S3::scheme() {
        return S3::from(&uri).map(S3::into_box);
    }

    let err = format!("Unknown remote uri scheme '{}'", uri.scheme());
    Err(Error::remote(err))
}
