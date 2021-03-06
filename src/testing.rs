use std::path::Path;

use tempfile::{self, NamedTempFile, TempDir};

pub const FIXTURES_PATH: &str = "tests/fixtures/snapshot";
pub const A_FILE_PATH: &str = "tests/fixtures/snapshot/a.txt";
pub const B_FILE_PATH: &str = "tests/fixtures/snapshot/b.txt";
pub const IS_SYMLINK_PATH: &str = "tests/fixtures/snapshot/is_symlink";
pub const IS_DIR_PATH: &str = "tests/fixtures/snapshot/is_dir";
pub const IS_BIN_PATH: &str = "tests/fixtures/snapshot/is_bin";
pub const TEAMCITY_BUILD_PROPS_PATH: &str = "tests/fixtures/teamcity/build.properties";
pub const TEAMCITY_CONFIG_PROPS_PATH: &str = "tests/fixtures/teamcity/config.properties";

#[derive(Debug)]
pub struct FileGuard(Option<NamedTempFile>);

#[derive(Debug)]
pub struct DirGuard(Option<TempDir>);

impl AsRef<Path> for FileGuard {
    fn as_ref(&self) -> &Path {
        match self.0 {
            Some(ref temp) => temp.as_ref(),
            None => panic!("using after close"),
        }
    }
}

impl AsRef<Path> for DirGuard {
    fn as_ref(&self) -> &Path {
        match self.0 {
            Some(ref temp) => temp.path(),
            None => panic!("using after close"),
        }
    }
}

impl Drop for FileGuard {
    fn drop(&mut self) {
        if let Some(file) = self.0.take() {
            file.close().expect("cannot close temporary file")
        }
    }
}

impl Drop for DirGuard {
    fn drop(&mut self) {
        if let Some(dir) = self.0.take() {
            dir.close().expect("cannot close temporary file")
        }
    }
}

pub fn temp_file(suffix: &str) -> FileGuard {
    let mut b = tempfile::Builder::new();
    let file = b.suffix(suffix).tempfile().unwrap();
    FileGuard(Some(file))
}

pub fn temp_dir() -> DirGuard {
    let b = tempfile::Builder::new();
    let dir = b.tempdir().unwrap();
    DirGuard(Some(dir))
}
