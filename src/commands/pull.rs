use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};

use log::{error, info, warn};
use serde::Serialize;
use serde_json;

use crate::errors::ResultExt;
use crate::snapshot::{Reading, Unpack};
use crate::{Config, Error, Stats, Storage};

#[derive(Debug)]
pub struct Pull<'a, 'b> {
    cfg: &'a Config,
    storage: &'b Storage,
    cached_dirs: Vec<PathBuf>,
    unpack_prefix: Option<PathBuf>,
}

impl<'a, 'b> Pull<'a, 'b> {
    pub fn new<P1, P2>(
        cfg: &'a Config,
        storage: &'b Storage,
        cached_dirs: &[P1],
        unpack_prefix: Option<P2>,
    ) -> Self
    where
        P1: AsRef<Path>,
        P2: AsRef<Path>,
    {
        Pull {
            cfg,
            storage,
            cached_dirs: cached_dirs
                .iter()
                .map(|it| it.as_ref().to_path_buf())
                .collect(),
            unpack_prefix: unpack_prefix.map(|it| it.as_ref().to_path_buf()),
        }
    }

    pub fn run(self) -> Result<(), Error> {
        let Self {
            cfg,
            storage,
            cached_dirs,
            unpack_prefix,
        } = self;

        if storage.is_downloable() {
            if let Err(err) = storage.download(&cfg.snapshot_file) {
                if cfg.verbose {
                    error!("{:?}", err);
                } else {
                    error!("{}", err);
                }
            }
        }

        let cached_dirs = cached_dirs
            .into_iter()
            .filter_map(is_cacheable)
            .collect::<Vec<_>>();

        write_json(&cfg.cached_dirs_file, &cached_dirs)?;

        if !cfg.snapshot_file.exists() {
            warn!("The previous snapshot wasn't found");
            return Ok(());
        }

        info!("Unpacking snapshot ...");

        let (entries, _) = {
            let _timer = Stats::current().unpacking().timer();
            let snapshot = Reading::open(&cfg.snapshot_file)?;
            snapshot.unpack(unpack_prefix, &cached_dirs)?
        };

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

        let cfg = Config::from(work.as_ref()).unwrap();
        let storage = Storage::new(&cfg);
        let command = Pull::new(&cfg, &storage, &dirs, Some(dst));

        command.run().unwrap();
    }
}
