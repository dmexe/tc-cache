use std::fs::File;
use std::path::{Path, PathBuf};

use log::{warn, info};

use crate::errors::ResultExt;
use crate::snapshot::{Pack, Writing};
use crate::Config;
use crate::Error;

pub struct Push<'a> {
    cfg: &'a Config,
}

impl<'a> Push<'a> {
    pub fn new(cfg: &'a Config) -> Self {
        Push { cfg }
    }

    pub fn run(self) -> Result<Vec<PathBuf>, Error> {
        let Self { cfg } = self;

        let cached_dirs = read_cached_dirs(&cfg.cached_dirs_file)?;
        if cached_dirs.is_empty() {
            warn!("No cached directories found, exiting");
            return Ok(cached_dirs);
        }

        if !cfg.cached_entries_file.exists() {
            warn!("No files from a previous snapshot, assume it isn't cached before");
        }
        
        info!("Creating a new snapshot ...");

        let snapshot = Writing::open(&cfg.snapshot_file)?;
        snapshot.pack(&cached_dirs)?;

        Ok(cached_dirs)
    }
}

fn read_cached_dirs(path: &Path) -> Result<Vec<PathBuf>, Error> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let reader = File::open(&path).io_err(&path)?;
    let cached_dirs = serde_json::from_reader::<_, Vec<PathBuf>>(&reader).io_err(&path)?;

    Ok(cached_dirs)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::commands::Pull;
    use crate::testing::{self, FIXTURES_PATH};

    #[test]
    fn push() {
        let work = testing::temp_dir();
        let dst = testing::temp_dir();
        let cfg = Config::from(&work).unwrap();
        let dirs = vec![PathBuf::from(FIXTURES_PATH)];
        let pull = Pull::new(&cfg, dirs.clone(), Some(dst));
        let push = Push::new(&cfg);

        pull.run().unwrap();

        let actual = push.run().unwrap();
        let expected = dirs
            .iter()
            .map(|it| it.canonicalize().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(actual, expected);
    }
}
