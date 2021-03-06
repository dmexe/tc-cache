use std::cmp::PartialEq;
use std::convert::From;
use std::convert::TryInto;
use std::fmt::Display;
use std::fs;
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::os::unix::fs::MetadataExt as UnixMetadata;
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use log::{debug, error};
use rayon::prelude::ParallelIterator;
use serde_derive::{Deserialize, Serialize};
use walkdir::{DirEntry, WalkDir};

use crate::errors::ResultExt;
use crate::{hashing, Error, Stats};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, Eq)]
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

impl Hash for Attributes {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u32(self.mode)
    }
}

impl PartialEq<Attributes> for Attributes {
    #[inline]
    fn eq(&self, other: &Attributes) -> bool {
        self.mode == other.mode
    }
}

#[derive(Debug, PartialEq)]
pub enum EntryKind {
    File,
    Symlink,
    Dir,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone, Hash)]
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
        let len = len
            .try_into()
            .map_err(|err| err.to_string())
            .io_err(&path)?;

        Ok(Entry::File {
            path: path.as_ref().to_path_buf(),
            attr: attr.into(),
            md5: md5.into(),
            len,
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

    pub fn walk<P>(dirs: &[P]) -> impl ParallelIterator<Item = Result<Entry, Error>>
    where
        P: AsRef<Path>,
    {
        use rayon::prelude::*;

        let dirs: Vec<PathBuf> = dirs.iter().map(|it| it.as_ref().to_path_buf()).collect();
        let (tx, rx) = mpsc::channel();

        rayon::spawn(move || {
            for dir in dirs {
                let walker = WalkDir::new(&dir).follow_links(false).max_open(256);

                for item in walker {
                    debug!("walk {:?}", item);
                    Stats::current().walking().inc(1);

                    let is_err = item.is_err();
                    let item = item.map(DirEntry::into_path).io_err(&dir);

                    if tx.send(item).is_err() {
                        error!("Cannot send entry into channel (is consumer dead?), exiting");
                        return;
                    }

                    if is_err {
                        return;
                    }
                }
            }
        });

        rx.into_iter()
            .par_bridge()
            .map(move |it| it.and_then(Entry::try_from_path))
    }

    pub fn walk_into_vec<P>(dirs: &[P]) -> Result<Vec<Entry>, Error>
    where
        P: AsRef<Path>,
    {
        use rayon::prelude::*;

        type Memo = Vec<Entry>;
        type Item = Result<Entry, Error>;

        let folder = |mut memo: Memo, item: Item| {
            debug!("fold {:?}", item);
            let item = item?;
            memo.push(item);
            Ok(memo)
        };

        let reducer = |mut memo: Memo, item: Memo| {
            memo.extend(item);
            Ok(memo)
        };

        let mut entries: Memo = Entry::walk(dirs)
            .try_fold(Vec::new, folder)
            .try_reduce(Vec::new, reducer)?;

        entries.sort_by_key(|it| it.as_ref().to_path_buf());

        Ok(entries)
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

    pub fn as_path(&self) -> &Path {
        match &self {
            Entry::File { path, .. } => path.as_path(),
            Entry::Symlink { path, .. } => path.as_path(),
            Entry::Dir { path, .. } => path.as_path(),
        }
    }

    pub fn as_attr(&self) -> &Attributes {
        match self {
            Entry::Dir { ref attr, .. } => &attr,
            Entry::Symlink { ref attr, .. } => &attr,
            Entry::File { ref attr, .. } => &attr,
        }
    }
}

impl AsRef<Path> for Entry {
    #[inline]
    fn as_ref(&self) -> &Path {
        self.as_path()
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

        let dirs = vec![FIXTURES_PATH, IS_DIR_PATH];
        let mut actual = Entry::walk_into_vec(&dirs)
            .unwrap()
            .iter()
            .map(|it| {
                let kind = it.kind();
                let md5 = it.as_md5().map(String::from).unwrap_or_default();
                let path = it.as_path().to_path_buf();
                (kind, md5, path)
            })
            .collect::<Vec<_>>();

        actual.sort_by_key(|it| it.2.clone());

        #[rustfmt::skip]
        let expected = vec! {
            (Dir,     "".into(),                                 "tests/fixtures/snapshot".into()),
            (File,    "0cc175b9c0f1b6a831c399e269772661".into(), "tests/fixtures/snapshot/a.txt".into()),
            (File,    "54510be579370aa078fbb9c5d9eed53a".into(), "tests/fixtures/snapshot/b.txt".into()),
            (File,    "33e4fd94e2560e008e2c3b431d0e3419".into(), "tests/fixtures/snapshot/is_bin".into()),
            (Dir,     "".into(),                                 "tests/fixtures/snapshot/is_dir".into()),
            (Dir,     "".into(),                                 "tests/fixtures/snapshot/is_dir".into()),
            (File,    "d41d8cd98f00b204e9800998ecf8427e".into(), "tests/fixtures/snapshot/is_dir/.keep".into()),
            (File,    "d41d8cd98f00b204e9800998ecf8427e".into(), "tests/fixtures/snapshot/is_dir/.keep".into()),
            (Symlink, "".into(),                                 "tests/fixtures/snapshot/is_symlink".into())
        };

        assert_eq!(actual, expected)
    }
}
