use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use crate::errors::ResultExt;
use crate::Error;

const WORK_DIR: &str = ".tc-cache";

#[derive(Debug)]
pub struct Config {
    pub working_dir: PathBuf,
    pub cached_dirs_file: PathBuf,
    pub cached_entries_file: PathBuf,
    pub snapshot_file: PathBuf,
    pub storage_file: PathBuf,
    pub verbose: bool,
}

impl Config {
    pub fn from_env() -> Result<Self, Error> {
        let home = env::var("HOME").unwrap_or_else(|_| ".".into());
        let working_dir = Path::new(home.as_str()).join(WORK_DIR);
        Config::from(working_dir)
    }

    pub fn from<W>(working_dir: W) -> Result<Self, Error>
    where
        W: AsRef<Path>,
    {
        let working_dir = working_dir.as_ref().to_path_buf();

        if !working_dir.exists() {
            fs::create_dir_all(&working_dir).io_err(&working_dir)?;
        }

        let working_dir = working_dir.canonicalize().io_err(&working_dir)?;

        let mut cached_dirs_file = working_dir.clone();
        cached_dirs_file.push("cached_dirs.json");

        let mut cached_entries_file = working_dir.clone();
        cached_entries_file.push("cached_entries.json");

        let mut snapshot_file = working_dir.clone();
        snapshot_file.push(Config::snapshot_file_name());

        let mut storage_file = working_dir.clone();
        storage_file.push("storage.json");

        Ok(Config {
            working_dir,
            cached_dirs_file,
            cached_entries_file,
            snapshot_file,
            storage_file,
            verbose: false,
        })
    }

    pub fn snapshot_file_name() -> &'static str {
        "snapshot.snappy"
    }

    pub fn verbose(&mut self, verbose: bool) {
        self.verbose = verbose;
    }
}
