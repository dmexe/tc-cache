use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

use log::{error, info, warn};
use serde::Serialize;
use serde_json;

use crate::errors::ResultExt;
use crate::snapshot::{Reading, Unpack};
use crate::{Config, Error};

#[derive(Debug)]
pub struct Pull<'a> {
    cfg: &'a Config,
    cached_dirs: Vec<PathBuf>,
    unpack_prefix: Option<PathBuf>,
}

impl<'a> Pull<'a> {
    pub fn new(cfg: &'a Config, cached_dirs: Vec<PathBuf>) -> Self {
        Pull {
            cfg,
            cached_dirs,
            unpack_prefix: None,
        }
    }

    pub fn unpack_prefix<P: AsRef<Path>>(self, path: P) -> Pull<'a> {
        Pull {
            unpack_prefix: Some(path.as_ref().to_path_buf()),
            .. self
        }
    }

    pub fn run(self) -> Result<(), Error> {
        let Self {
            cfg,
            cached_dirs,
            unpack_prefix,
        } = self;

        let cached_dirs = cached_dirs
            .into_iter()
            .filter_map(is_cacheable)
            .collect::<Vec<_>>();

        write_json(&cfg.cached_dirs_file, &cached_dirs)?;

        if !cfg.snapshot_file.exists() {
            warn!(
                "A previous snapshot wasn't found at {:?}",
                cfg.snapshot_file.as_os_str()
            );
            return Ok(());
        }

        let snapshot = Reading::open(&cfg.snapshot_file)?;
        let (entries, _) = snapshot.unpack(unpack_prefix, &cached_dirs)?;

        write_json(&cfg.cached_entries_file, &entries)?;

        Ok(())
    }
}

fn write_json<T: Serialize>(path: &Path, item: &T) -> Result<(), Error> {
    let mut opts = OpenOptions::new();
    let file = opts
        .create(true)
        .truncate(true)
        .write(true)
        .open(&path)
        .io_err(&path)?;

    serde_json::to_writer(&file, item).io_err(&path)
}

fn is_cacheable<P: AsRef<Path>>(path: P) -> Option<PathBuf> {
    try_is_cacheable(path.as_ref()).ok().and_then(|it| it)
}

fn try_is_cacheable(path: &Path) -> Result<Option<PathBuf>, Error> {
    let log_err = |err| {
        error!("{}", err);
        err
    };

    if !path.exists() {
        info!("Path {:?} isn't exist, creating", path.as_os_str());
        fs::create_dir_all(&path).io_err(&path).map_err(log_err)?;
    }

    let path = path.canonicalize().io_err(&path).map_err(log_err)?;
    let meta = fs::symlink_metadata(&path).io_err(&path).map_err(log_err)?;

    if meta.file_type().is_symlink() {
        warn!("{:?} is a symlink, not following", path.as_os_str());
        return Ok(None);
    }

    info!("Add {:?} to cache", path.as_os_str());

    Ok(Some(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::testing::{self, FIXTURES_PATH};

    #[test]
    fn pull() {
        let work = testing::temp_dir();
        let dst = testing::temp_dir();
        let dirs = vec![PathBuf::from(FIXTURES_PATH)];

        let cfg = Config::new(work.as_ref()).unwrap();
        let command = Pull::new(&cfg, dirs).unpack_prefix(dst);

        command.run().unwrap();
    }
}