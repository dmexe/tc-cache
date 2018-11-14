use std::fs::{self, OpenOptions};
use std::io::Read;
use std::os::unix::fs::{self as unix_fs, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use filetime::{self, FileTime};

use crate::entries::Attr;
use crate::errors::ResultExt;
use crate::snapshot::{Reading, O_DIRECT};
use crate::{Entry, Error};

pub trait Unpack {
    fn unpack<P>(self, prefix: Option<PathBuf>, dirs: &[P]) -> Result<(Vec<Entry>, usize), Error>
    where
        P: AsRef<Path>;
}

impl<R: Read> Unpack for Reading<R> {
    fn unpack<P>(
        mut self,
        prefix: Option<PathBuf>,
        dirs: &[P],
    ) -> Result<(Vec<Entry>, usize), Error>
    where
        P: AsRef<Path>,
    {
        let prefixed = prefixed(prefix);
        let mut read: usize = 0;
        let mut entries = Vec::new();

        loop {
            let (entry, len) = match self.read_entry()? {
                Some(entry) => entry,
                None => break,
            };
            read += len;

            if !is_include(dirs, entry.as_ref()) {
                if let Some((_, _, _, size)) = entry.as_file() {
                    self.skip(size as usize)?;
                }
                continue;
            }

            if let Some((path, attr)) = entry.as_dir() {
                let path = prefixed(path);
                fs::create_dir_all(&path).io_err(&path)?;
                restore_attributes(&path, &attr)?;
            }

            if let Some((path, target, _)) = entry.as_symlink() {
                let path = prefixed(path);
                unix_fs::symlink(&target, &path).io_err(&path)?;
                // restore_attributes(&path, &attr) only for osx
            }

            if let Some((path, attr, _, size)) = entry.as_file() {
                let path = prefixed(path);
                let len = unpack_file(&mut self, &path, size as usize)?;
                restore_attributes(&path, &attr)?;

                read += len;
            }

            entries.push(entry);
        }

        Ok((entries, read))
    }
}

fn unpack_file<P, R>(snapshot: &mut Reading<R>, dst: P, len: usize) -> Result<usize, Error>
where
    P: AsRef<Path>,
    R: Read,
{
    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .custom_flags(O_DIRECT)
        .open(&dst)
        .io_err(&dst)?;

    snapshot.copy_to(&mut file, len)
}

fn restore_attributes<P>(path: P, attr: &Attr) -> Result<(), Error>
where
    P: AsRef<Path>,
{
    let meta = fs::symlink_metadata(&path).io_err(&path)?;
    let mut perm = meta.permissions();
    perm.set_mode(attr.mode);

    let atime = FileTime::from_unix_time(attr.atime, 0);
    let mtime = FileTime::from_unix_time(attr.mtime, 0);

    filetime::set_file_times(&path, atime, mtime).io_err(&path)
}

#[inline]
fn is_include<P>(dirs: &[P], path: &Path) -> bool
where
    P: AsRef<Path>,
{
    dirs.iter().any(|it| path.starts_with(it))
}

fn prefixed(prefix: Option<PathBuf>) -> impl Fn(&Path) -> PathBuf {
    move |path| match prefix {
        Some(ref prefix) => {
            let path = if path.is_absolute() {
                path.strip_prefix("/").unwrap()
            } else {
                path
            };
            let prefix = prefix.clone();
            prefix.join(path)
        }
        None => path.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::snapshot::{Pack, Writing};
    use crate::testing::{self, FIXTURES_PATH};

    #[test]
    fn is_include() {
        let dirs = vec![Path::new("/a"), Path::new("b")];
        let params = vec![("/a/name", true), ("b/name", true), ("/c/name", false)];

        for (path, expected) in params {
            let path = PathBuf::from(path);
            assert_eq!(super::is_include(&dirs, path.as_path()), expected);
        }
    }

    #[test]
    fn prefixed() {
        let params = vec![
            ("/a/prefix", "/b/file", "/a/prefix/b/file"),
            ("/a/prefix", "b/file", "/a/prefix/b/file"),
            ("a/prefix", "/b/file", "a/prefix/b/file"),
            ("a/prefix", "b/file", "a/prefix/b/file"),
        ];

        for (prefix, item, expected) in params {
            let prefix = PathBuf::from(prefix);
            let item = PathBuf::from(item);
            let expected = PathBuf::from(expected);
            let prefixed = super::prefixed(Some(prefix));

            assert_eq!(prefixed(item.as_path()), expected);
        }
    }

    #[test]
    fn unpack() {
        let src = testing::temp_file(".sn");
        let dst = testing::temp_dir();
        let dirs = vec![Path::new(FIXTURES_PATH)];

        let expected = {
            let snapshot = Writing::open(&src).unwrap();
            snapshot.pack(&dirs).unwrap()
        };

        let snapshot = Reading::open(&src).unwrap();
        let (_, actual) = snapshot
            .unpack(Some(dst.as_ref().to_path_buf()), &dirs)
            .unwrap();

        assert_eq!(expected, actual);
    }
}
