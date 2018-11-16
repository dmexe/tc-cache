use std::collections::HashSet;
use std::fs::File;
use std::path::{Path, PathBuf};

use log::{info, warn};
use memmap::MmapOptions;

use crate::errors::ResultExt;
use crate::snapshot::{self, Entry, Pack, Writing};
use crate::{Config, Error, Stats};

pub struct Push<'a> {
    cfg: &'a Config,
}

impl<'a> Push<'a> {
    pub fn new(cfg: &'a Config) -> Self {
        Push { cfg }
    }

    pub fn run(self) -> Result<(Vec<PathBuf>, Option<usize>), Error> {
        let Self { cfg } = self;
        let mut changed = true;
        let mut snapshot_len = None;

        let cached_dirs = read_cached_dirs(&cfg.cached_dirs_file)?;
        if cached_dirs.is_empty() {
            warn!("No cached directories found, exiting");
            return Ok((cached_dirs, snapshot_len));
        }

        let previous_entries = read_cached_entries(&cfg.cached_entries_file)?;
        let current_entries = {
            info!("Walking cached directories ...");
            let _timer = Stats::current().walking().timer();
            Entry::walk_into_vec(&cached_dirs)?
        };

        if previous_entries.is_empty() {
            warn!("No files from a previous snapshot, assume it isn't cached before");
        } else {
            let diff = snapshot::diff(&previous_entries, &current_entries);
            changed = detect_changes(&diff);
        }

        if changed {
            info!("Creating a new snapshot ...");
            {
                let _timer = Stats::current().packing().timer();
                let snapshot = Writing::open(&cfg.snapshot_file)?;
                snapshot.pack(&cached_dirs)?;
            }
            let meta = &cfg.snapshot_file.metadata().io_err(&cfg.snapshot_file)?;
            snapshot_len = Some(meta.len() as usize);
        }

        Ok((cached_dirs, snapshot_len))
    }
}

fn detect_changes(diff: &HashSet<&Path>) -> bool {
    let len = diff.len();
    let next = diff.iter().next();

    if let Some(next) = next {
        if len == 1 {
            info!("Changes detected; {:?}", next);
        } else {
            info!("Changed detected; {:?} plus {} files", next, len - 1);
        }
    } else {
        info!("No changes detected");
    }

    next.is_some()
}

fn read_cached_entries(path: &Path) -> Result<Vec<Entry>, Error> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    info!("Reading previously cached entries ...");

    let file = File::open(&path).io_err(&path)?;
    let mmap = MmapOptions::new();
    let mmap = unsafe { mmap.map(&file) };
    let mmap = mmap.io_err(&path)?;

    serde_json::from_slice(mmap.as_ref()).io_err(&path)
}

fn read_cached_dirs(path: &Path) -> Result<Vec<PathBuf>, Error> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    let reader = File::open(&path).io_err(&path)?;
    serde_json::from_reader::<_, Vec<PathBuf>>(&reader).io_err(&path)
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

        let (actual, _) = push.run().unwrap();
        let expected = dirs
            .iter()
            .map(|it| it.canonicalize().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(actual, expected);
    }
}
