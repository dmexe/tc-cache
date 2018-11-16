use std::fs::File;
use std::io::Write;
use std::path::Path;

use crate::errors::ResultExt;
use crate::snapshot::{Entry, Writing};
use crate::Error;

pub trait Pack {
    fn pack<P>(self, dirs: &[P]) -> Result<usize, Error>
    where
        P: AsRef<Path>;
}

impl<W: Write> Pack for Writing<W> {
    fn pack<P>(mut self, dirs: &[P]) -> Result<usize, Error>
    where
        P: AsRef<Path>,
    {
        let mut written: usize = 0;

        for entry in read_entries(dirs)? {
            written += self.write_entry(&entry)?;

            if let Some((path, _, _, size)) = entry.as_file() {
                let size = size as usize;
                let mut file = File::open(&path).io_err(&path)?;
                written += self.write_file(&mut file, &path, size)?;
            }
        }
        self.flush()?;

        Ok(written)
    }
}

fn read_entries<P>(dirs: &[P]) -> Result<Vec<Entry>, Error>
where
    P: AsRef<Path>,
{
    use rayon::prelude::*;

    type Memo = Vec<Entry>;
    type Item = Result<Entry, Error>;

    let folder = |mut memo: Memo, item: Item| {
        let item = item?;
        memo.push(item);
        Ok(memo)
    };

    let reducer = |mut memo: Memo, item: Memo| {
        memo.extend(item);
        Ok(memo)
    };

    let dirs: Vec<&Path> = dirs.iter().map(|it| it.as_ref()).collect();

    let mut entries: Memo = dirs
        .into_par_iter()
        .flat_map(Entry::walk)
        .try_fold(Vec::new, folder)
        .try_reduce(Vec::new, reducer)?;

    entries.sort_by_key(|it| it.as_ref().to_path_buf());

    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::testing::{temp_file, FIXTURES_PATH, IS_DIR_PATH};

    #[test]
    fn read_entries() {
        use crate::snapshot::EntryKind::*;

        let src = vec![Path::new(FIXTURES_PATH), Path::new(IS_DIR_PATH)];
        let entries = super::read_entries(&src).unwrap();
        let entries = entries
            .iter()
            .map(|it| {
                let kind = it.kind();
                let md5 = it.as_md5().map(String::from).unwrap_or_default();
                let path = it.as_ref().to_path_buf();
                (kind, md5, path)
            })
            .collect::<Vec<_>>();

        #[rustfmt::skip]
        let expected = vec! {
            (Dir,     "".into(),                                 "tests/fixtures".into()),
            (File,    "0cc175b9c0f1b6a831c399e269772661".into(), "tests/fixtures/a.txt".into()),
            (File,    "54510be579370aa078fbb9c5d9eed53a".into(), "tests/fixtures/b.txt".into()),
            (Dir,     "".into(),                                 "tests/fixtures/is_dir".into()),
            (Dir,     "".into(),                                 "tests/fixtures/is_dir".into()),
            (File,    "d41d8cd98f00b204e9800998ecf8427e".into(), "tests/fixtures/is_dir/.keep".into()),
            (File,    "d41d8cd98f00b204e9800998ecf8427e".into(), "tests/fixtures/is_dir/.keep".into()),
            (Symlink, "".into(),                                 "tests/fixtures/is_symlink".into())
        };

        assert_eq!(entries, expected)
    }

    #[test]
    fn pack() {
        let dst = temp_file(".sn");
        let src = vec![Path::new(FIXTURES_PATH), Path::new(IS_DIR_PATH)];

        let snapshot = Writing::open(&dst).unwrap();
        let written = snapshot.pack(&src).unwrap();

        assert_eq!(written, 83732);
    }
}
