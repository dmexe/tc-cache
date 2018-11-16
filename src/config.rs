use std::fs;
use std::path::{Path, PathBuf};

use crate::errors::ResultExt;
use crate::Error;

#[derive(Debug)]
pub struct Config {
    pub working_dir: PathBuf,
    pub cached_dirs_file: PathBuf,
    pub cached_entries_file: PathBuf,
    pub snapshot_file: PathBuf,
}

impl Config {
    pub fn new<W>(working_dir: W) -> Result<Self, Error>
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
        snapshot_file.push("snapshot.snappy");

        Ok(Config {
            working_dir,
            cached_dirs_file,
            cached_entries_file,
            snapshot_file,
        })
    }
}