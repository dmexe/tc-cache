use std::path::Path;

use url::Url;

use crate::{Error, Stats, Config};

mod backend;
mod futures_ext;

#[derive(Debug)]
pub struct Remote {
    backend: Option<Box<dyn backend::Backend>>,
    key_prefix: Option<String>,
}

impl Remote {
    pub fn new(_cfg: &Config) -> Self {
        Self {
            backend: None,
            key_prefix: None
        }
    }
    
    pub fn uri<S>(&mut self, uri: S) -> Result<Self, Error>
    where
        S: AsRef<str>,
    {
        let uri = Url::parse(uri.as_ref()).map_err(Error::remote)?;

        if uri.scheme() == backend::S3::scheme() {
            let s3 = backend::S3::from(&uri)?;
            self.backend = Some(Box::new(s3))
        }

        let err = format!("Unknown remote uri '{}'", uri);
        Err(Error::remote(err))
    }

    pub fn prefix<S>(&mut self, key: S)
    where
        S: AsRef<str>,
    {
        let key_prefix = match &self.key_prefix {
            Some(val) => format!("{}/{}", val, key.as_ref()),
            None => key.as_ref().to_string(),
        };
        
        self.key_prefix = Some(key_prefix);
    }
    
    pub fn is_empty(&self) -> bool {
        self.backend.is_none()
    }

    pub fn download<P>(&self, path: P) -> Result<(), Error>
    where
        P: AsRef<Path>,
    {
        let inner = match &self.backend {
            Some(val) => val,
            None => return Ok(())
        };
        
        let _timer = Stats::current().download();
        let file_name = file_name(&path)?;
        let file_name = self.prefixed(file_name);

        let req = backend::DownloadRequest {
            path: path.as_ref().to_path_buf(),
            key: file_name,
        };

        let len = inner.download(req)?;
        Stats::current().download().inc(len);

        Ok(())
    }

    pub fn upload<P>(&self, path: P, len: usize) -> Result<(), Error>
    where
        P: AsRef<Path>,
    {
        let inner = match &self.backend {
            Some(val) => val,
            None => return Ok(())
        };
        
        let _timer = Stats::current().upload();
        let file_name = file_name(&path)?;
        let file_name = self.prefixed(file_name);

        let req = backend::UploadRequest {
            path: path.as_ref().to_path_buf(),
            key: file_name,
            len,
        };

        let len = inner.upload(req)?;
        Stats::current().upload().inc(len);

        Ok(())
    }

    fn prefixed<S>(&self, key: S) -> String
    where
        S: AsRef<str>,
    {
        if let Some(prefix) = &self.key_prefix {
            format!("{}/{}", prefix, key.as_ref())
        } else {
            key.as_ref().to_string()
        }
    }
}

fn file_name<P>(path: P) -> Result<String, Error>
where
    P: AsRef<Path>,
{
    path.as_ref()
        .file_name()
        .and_then(|it| it.to_str())
        .map(|it| it.to_string())
        .ok_or_else(|| {
            let err = format!("Empty file name for {:?}", path.as_ref());
            Error::remote(err)
        })
}
