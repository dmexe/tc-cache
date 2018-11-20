use std::io::Write;
use std::path::Path;

use crate::snapshot::{Entry, Writing};
use crate::Error;

pub trait Pack {
    fn pack<P>(self, dirs: &[P]) -> Result<usize, Error>
    where
        P: AsRef<Path>;

    fn pack_with_entries(self, entries: &[Entry]) -> Result<usize, Error>;
}

impl<W: Write> Pack for Writing<W> {
    fn pack<P>(self, dirs: &[P]) -> Result<usize, Error>
    where
        P: AsRef<Path>,
    {
        let entries = Entry::walk_into_vec(&dirs)?;
        self.pack_with_entries(&entries)
    }

    fn pack_with_entries(mut self, entries: &[Entry]) -> Result<usize, Error> {
        let mut written = 0_usize;

        for entry in entries {
            written += self.write_entry(&entry)?;

            if let Some((path, _, _, len)) = entry.as_file() {
                if len > 0 {
                    written += self.write_file(&path, Some(len))?;
                }
            }
        }
        self.flush()?;

        Ok(written)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::testing::{temp_file, FIXTURES_PATH, IS_DIR_PATH};

    #[test]
    fn pack() {
        let dst = temp_file(".sn");
        let src = vec![Path::new(FIXTURES_PATH), Path::new(IS_DIR_PATH)];

        let snapshot = Writing::open(&dst).unwrap();
        let written = snapshot.pack(&src).unwrap();

        assert_eq!(written, 83804);
    }
}
