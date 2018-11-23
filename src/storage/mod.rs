use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use serde_json::{self, json, Value};
use url::Url;

use crate::errors::ResultExt;
use crate::{Config, Error, Stats};

mod backend;
mod futures_ext;

#[derive(Debug, Default)]
pub struct Storage {
    backend: Option<Box<dyn backend::Backend>>,
    uri: Option<String>,
    key_prefix: Option<String>,
    path: PathBuf,
    uploadable: bool,
}

impl Storage {
    pub fn new(cfg: &Config) -> Self {
        Self {
            path: cfg.storage_file.clone(),
            ..Default::default()
        }
    }

    pub fn uri<S>(&mut self, uri: S) -> Result<(), Error>
    where
        S: AsRef<str>,
    {
        let uri = Url::parse(uri.as_ref()).map_err(Error::storage)?;

        if uri.scheme() == backend::S3::scheme() {
            let s3 = backend::S3::from(&uri)?;
            self.backend = Some(Box::new(s3));
            self.uri = Some(uri.as_ref().to_string());
            return Ok(());
        }

        let err = format!("Unknown remote uri '{}'", uri);
        Err(Error::storage(err))
    }

    pub fn key_prefix<S>(&mut self, key: S)
    where
        S: AsRef<str>,
    {
        let key_prefix = match &self.key_prefix {
            Some(val) => format!("{}/{}", val, key.as_ref()),
            None => key.as_ref().to_string(),
        };

        self.key_prefix = Some(key_prefix);
    }

    pub fn uploadable(&mut self, uploadable: bool) {
        self.uploadable = uploadable;
    }

    pub fn is_uploadable(&self) -> bool {
        self.backend.is_some() && self.uploadable
    }

    pub fn is_downloable(&self) -> bool {
        self.backend.is_some()
    }

    pub fn download<P>(&self, path: P) -> Result<(), Error>
    where
        P: AsRef<Path>,
    {
        let inner = match &self.backend {
            Some(val) => val,
            None => return Ok(()),
        };

        let _timer = Stats::current().download();
        let file_name = file_name(&path)?;
        let file_name = self.key_prefixed(file_name);

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
            None => return Ok(()),
        };

        let _timer = Stats::current().upload();
        let file_name = file_name(&path)?;
        let file_name = self.key_prefixed(file_name);

        let req = backend::UploadRequest {
            path: path.as_ref().to_path_buf(),
            key: file_name,
            len,
        };

        let len = inner.upload(req)?;
        Stats::current().upload().inc(len);

        Ok(())
    }

    pub fn key_prefixed<S>(&self, key: S) -> String
    where
        S: AsRef<str>,
    {
        if let Some(prefix) = &self.key_prefix {
            format!("{}/{}", prefix, key.as_ref())
        } else {
            key.as_ref().to_string()
        }
    }

    pub fn save(&self) -> Result<(), Error> {
        let content = json!({
            "uri": self.uri,
            "key_prefix": self.key_prefix,
            "uploadable": self.uploadable,
        });

        let mut opts = OpenOptions::new();
        let file = opts
            .write(true)
            .truncate(true)
            .create(true)
            .open(&self.path)
            .io_err(&self.path)?;

        serde_json::to_writer(&file, &content).io_err(&self.path)?;
        Ok(())
    }

    pub fn load<P>(path: P) -> Result<Self, Error>
    where
        P: AsRef<Path>,
    {
        let file = File::open(&path).io_err(&path)?;
        let value: Value = serde_json::from_reader(&file).io_err(&path)?;

        let obj = match value.as_object() {
            Some(obj) => obj,
            None => return Err(Error::storage("Cannot found root object in json")),
        };

        let mut storage = Storage {
            path: path.as_ref().to_path_buf(),
            ..Default::default()
        };

        if let Some(uri) = obj.get("uri").and_then(|it| it.as_str()) {
            storage.uri(&uri)?;
        }

        if let Some(key_prefix) = obj.get("key_prefix").and_then(|it| it.as_str()) {
            storage.key_prefix(&key_prefix);
        }

        if let Some(uploadable) = obj.get("uploadable").and_then(|it| it.as_bool()) {
            storage.uploadable(uploadable);
        }

        Ok(storage)
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
            Error::storage(err)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::testing;

    #[test]
    fn prefix() {
        let work = testing::temp_dir();
        let cfg = Config::from(work.as_ref()).unwrap();
        let mut storage = Storage::new(&cfg);

        assert_eq!(storage.key_prefixed("foo"), "foo");

        storage.key_prefix("bar");

        assert_eq!(storage.key_prefixed("foo"), "bar/foo");
    }

    #[test]
    fn save() {
        let work = testing::temp_dir();
        let cfg = Config::from(work.as_ref()).unwrap();
        let mut storage = Storage::new(&cfg);

        storage.uri("s3://bucket/prefix").unwrap();
        storage.key_prefix("prefix");
        storage.save().unwrap();

        let _storage = Storage::load(cfg.storage_file).unwrap();
    }
}
