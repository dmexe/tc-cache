use std::collections::HashSet;
use std::fs::File;
use std::path::{Path, PathBuf};

use log::{error, info, warn};

use crate::errors::ResultExt;
use crate::snapshot::{self, Diff, Entry, Pack, Writing};
use crate::{mmap, Config, Error, Stats, Remote};

pub struct Push<'a, 'b> {
    cfg: &'a Config,
    remote: &'b Remote,
}

impl<'a, 'b> Push<'a, 'b> {
    pub fn new(cfg: &'a Config, remote: &'b Remote) -> Self {
        Push { cfg, remote }
    }

    pub fn run(self) -> Result<(Vec<PathBuf>, Option<usize>), Error> {
        let Self { cfg, remote } = self;
        let mut changed = true;

        let cached_dirs = read_cached_dirs(&cfg.cached_dirs_file)?;
        if cached_dirs.is_empty() {
            warn!("No cached directories found, exiting");
            return Ok((cached_dirs, None));
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
            changed = detect_changes(&diff, cfg.verbose);
        }

        if !changed {
            return Ok((cached_dirs, None));
        }

        info!("Creating a new snapshot ...");
        {
            let _timer = Stats::current().packing().timer();
            let snapshot = Writing::open(&cfg.snapshot_file)?;
            snapshot.pack(&cached_dirs)?;
        }

        let meta = &cfg.snapshot_file.metadata().io_err(&cfg.snapshot_file)?;
        let len = meta.len() as usize;

        if !remote.is_empty() {
            info!("Attempting to upload snapshot ...");

            if let Err(err) = remote.upload(&cfg.snapshot_file, len) {
                error!("{}", err);
            }
        }

        Ok((cached_dirs, Some(len)))
    }
}

fn detect_changes(diff: &HashSet<Diff>, verbose: bool) -> bool {
    let next = match diff.iter().next() {
        Some(val) => val,
        None => {
            info!("No changes detected");
            return false;
        }
    };

    if verbose {
        detect_changes_verbose(&diff);
    } else {
        let len = diff.len();
        if len == 1 {
            info!("Changes detected; {:?}", next.as_path());
        } else {
            info!(
                "Changed detected; {:?} plus {} files",
                next.as_path(),
                len - 1
            );
        }
    }

    true
}

fn detect_changes_verbose(diff: &HashSet<Diff>) {
    for it in diff {
        info!("{}", it);
    }
}

fn read_cached_entries(path: &Path) -> Result<Vec<Entry>, Error> {
    if !path.exists() {
        return Ok(Vec::new());
    }

    info!("Reading previously cached entries ...");

    let (_, _, src) = mmap::read(&path, None)?;
    serde_json::from_slice(&src).io_err(&path)
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
        let remote = Remote::new(&cfg);

        let dirs = vec![PathBuf::from(FIXTURES_PATH)];
        let pull = Pull::new(&cfg, &remote, dirs.clone(), Some(dst));
        let push = Push::new(&cfg, &remote);

        pull.run().unwrap();

        let (actual, _) = push.run().unwrap();
        let expected = dirs
            .iter()
            .map(|it| it.canonicalize().unwrap())
            .collect::<Vec<_>>();

        assert_eq!(actual, expected);
    }
}
