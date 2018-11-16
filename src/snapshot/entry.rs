use std::convert::From;
use std::fs;
use std::fs::File;
use std::os::unix::fs::MetadataExt as UnixMetadata;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::convert::TryInto;
use std::fmt::Display;

use log::error;
use rayon::prelude::ParallelIterator;
use serde_derive::{Deserialize, Serialize};
use walkdir::{DirEntry, WalkDir};

use crate::errors::ResultExt;
use crate::hashing;
use crate::Error;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Attributes {
    pub mode: u32,
    pub atime: i64,
    pub mtime: i64,
}

impl Attributes {
    pub fn new(mode: u32, atime: i64, mtime: i64) -> Self {
        Attributes { mode, atime, mtime }
    }
}

impl<T: UnixMetadata> From<T> for Attributes {
    fn from(metadata: T) -> Self {
        Attributes::new(metadata.mode(), metadata.atime(), metadata.mtime())
    }
}

#[derive(Debug, PartialEq)]
pub enum EntryKind {
    File,
    Symlink,
    Dir,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "_t")]
pub enum Entry {
    #[serde(rename = "f")]
    File {
        path: PathBuf,
        attr: Attributes,
        md5: String,
        len: u32,
    },
    #[serde(rename = "s")]
    Symlink {
        path: PathBuf,
        target: PathBuf,
        attr: Attributes,
    },
    #[serde(rename = "d")]
    Dir { path: PathBuf, attr: Attributes },
}

impl Entry {
    pub fn file<P, A, M, L>(path: P, attr: A, md5: M, len: L) -> Result<Self, Error>
    where
        P: AsRef<Path>,
        A: Into<Attributes>,
        M: Into<String>,
        L: TryInto<u32>,
        L::Error: Display + Sized,
    {
        let len = len.try_into().map_err(|err| err.to_string()).io_err(&path)?;

        Ok(Entry::File {
            path: path.as_ref().to_path_buf(),
            attr: attr.into(),
            md5: md5.into(),
            len
        })
    }

    pub fn symlink<P, T, A>(path: P, target: T, attr: A) -> Self
    where
        P: AsRef<Path>,
        T: AsRef<Path>,
        A: Into<Attributes>,
    {
        Entry::Symlink {
            path: path.as_ref().to_path_buf(),
            target: target.as_ref().to_path_buf(),
            attr: attr.into(),
        }
    }

    pub fn dir<P, A>(path: P, attr: A) -> Self
    where
        P: AsRef<Path>,
        A: Into<Attributes>,
    {
        Entry::Dir {
            path: path.as_ref().to_path_buf(),
            attr: attr.into(),
        }
    }

    pub fn walk<P>(path: P) -> impl ParallelIterator<Item = Result<Entry, Error>>
    where
        P: AsRef<Path>,
    {
        use rayon::prelude::*;

        let walker = WalkDir::new(&path).follow_links(false).max_open(256);

        let (tx, rx) = mpsc::channel();
        let path = path.as_ref().to_path_buf();

        rayon::spawn(move || {
            for it in walker {
                let it = it.map(DirEntry::into_path);

                if tx.send(it).is_err() {
                    error!("Cannot send entry into channel (is consumer dead?), exiting");
                    return;
                }
            }
        });

        rx.into_iter()
            .par_bridge()
            .map(move |it| it.io_err(&path).and_then(Entry::try_from_path))
    }

    pub fn try_from_path<T>(path: T) -> Result<Self, Error>
    where
        T: AsRef<Path>,
    {
        let path = path.as_ref();
        let meta = fs::symlink_metadata(&path).io_err(&path)?; // no follow symlinks
        let file_type = meta.file_type();

        if file_type.is_symlink() {
            let target = fs::read_link(path).io_err(&path)?;
            return Ok(Entry::symlink(path, target, meta));
        }

        if file_type.is_dir() {
            return Ok(Entry::dir(path, meta));
        }

        if file_type.is_file() {
            let file = File::open(path).io_err(&path)?;
            let len = meta.len() as usize;
            let md5 = hashing::md5::file(file, len as usize).io_err(&path)?;
            return Entry::file(path, meta, md5, len);
        }

        let err = "Unknown file type, neither of a file nor a directory nor a symlink";
        Err(Error::io(path)(err))
    }

    pub fn as_file(&self) -> Option<(&Path, &Attributes, &str, usize)> {
        match self {
            Entry::File {
                ref path,
                ref attr,
                ref md5,
                len,
            } => Some((path.as_path(), attr, md5.as_str(), *len as usize)),
            _ => None,
        }
    }

    pub fn as_symlink(&self) -> Option<(&Path, &Path, &Attributes)> {
        match self {
            Entry::Symlink {
                ref path,
                ref target,
                ref attr,
            } => Some((path.as_path(), target.as_path(), attr)),
            _ => None,
        }
    }

    pub fn as_dir(&self) -> Option<(&Path, &Attributes)> {
        match self {
            Entry::Dir { ref path, ref attr } => Some((path.as_path(), attr)),
            _ => None,
        }
    }

    pub fn kind(&self) -> EntryKind {
        match &self {
            Entry::File { .. } => EntryKind::File,
            Entry::Symlink { .. } => EntryKind::Symlink,
            Entry::Dir { .. } => EntryKind::Dir,
        }
    }

    pub fn as_md5(&self) -> Option<&str> {
        match &self {
            Entry::File { md5, .. } => Some(md5.as_str()),
            Entry::Symlink { .. } => None,
            Entry::Dir { .. } => None,
        }
    }
}

impl AsRef<Path> for Entry {
    fn as_ref(&self) -> &Path {
        match &self {
            Entry::File { path, .. } => path.as_path(),
            Entry::Symlink { path, .. } => path.as_path(),
            Entry::Dir { path, .. } => path.as_path(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{A_FILE_PATH, FIXTURES_PATH, IS_DIR_PATH, IS_SYMLINK_PATH};

    #[test]
    fn entry_from_path() {
        let file = Entry::try_from_path(A_FILE_PATH).unwrap();
        let (path, _, md5, len) = file.as_file().unwrap();

        assert_eq!(path.as_os_str(), A_FILE_PATH);
        assert_eq!(len, 1);
        assert_eq!(md5, "0cc175b9c0f1b6a831c399e269772661");
    }

    #[test]
    fn entry_from_symlink() {
        let file = Entry::try_from_path(IS_SYMLINK_PATH).unwrap();
        let (path, target, _) = file.as_symlink().unwrap();

        assert_eq!(path.as_os_str(), IS_SYMLINK_PATH);
        assert_eq!(target.as_os_str(), "a.txt");
    }

    #[test]
    fn entry_from_dir() {
        let file = Entry::try_from_path(IS_DIR_PATH).unwrap();
        let (path, _) = file.as_dir().unwrap();

        assert_eq!(path.as_os_str(), IS_DIR_PATH);
    }
    
    #[test]
    fn check_file_size() {
        let path = Path::new(A_FILE_PATH);
        let meta = path.metadata().unwrap();
        let attr = Attributes::from(meta);
        let err = Entry::file(&path, attr, "", (::std::u32::MAX as u64) + 1).unwrap_err();
        
        assert!(err.to_string().contains("out of range"));
    }

    #[test]
    fn walk_directory() {
        use super::EntryKind::*;

        let mut actual = Entry::walk(FIXTURES_PATH)
            .map(|it| it.unwrap())
            .map(|it| {
                let kind = it.kind();
                let md5 = it.as_md5().map(String::from).unwrap_or_default();
                let path = it.as_ref().to_path_buf();
                (kind, md5, path)
            })
            .collect::<Vec<_>>();

        actual.sort_by_key(|it| it.2.clone());

        #[rustfmt::skip]
        let expected = vec! {
            (Dir,     "".into(),                                 "tests/fixtures".into()),
            (File,    "0cc175b9c0f1b6a831c399e269772661".into(), "tests/fixtures/a.txt".into()),
            (File,    "54510be579370aa078fbb9c5d9eed53a".into(), "tests/fixtures/b.txt".into()),
            (Dir,     "".into(),                                 "tests/fixtures/is_dir".into()),
            (File,    "d41d8cd98f00b204e9800998ecf8427e".into(), "tests/fixtures/is_dir/.keep".into()),
            (Symlink, "".into(),                                 "tests/fixtures/is_symlink".into())
        };

        assert_eq!(actual, expected)
    }
}
