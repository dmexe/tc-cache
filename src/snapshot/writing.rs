use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;

use crate::bytes::IntoLeBytes;
use crate::errors::ResultExt;
use crate::snapshot::{Entry, MEM_MAP_THRESHOLD, VERSION};
use crate::{Error, Stats};

#[derive(Debug)]
pub struct Writing<W = ()> {
    writer: W,
}

impl Writing {
    pub fn from<W: Write>(writer: W) -> Result<Writing<snap::Writer<W>>, Error> {
        let writer = snap::Writer::new(writer);
        let mut writer = Writing { writer };

        writer.write_version().map(|_| writer)
    }

    pub fn open<P: AsRef<Path>>(path: P) -> Result<Writing<snap::Writer<File>>, Error> {
        let file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&path)
            .io_err(&path)?;

        Writing::from(file)
    }
}

impl<W: Write> Writing<W> {
    fn write_version(&mut self) -> Result<(), Error> {
        Stats::current().packing().inc(VERSION.len());

        self.writer
            .write_all(VERSION)
            .snapshot_err("Write version header failed")
    }

    pub fn flush(&mut self) -> Result<(), Error> {
        self.writer.flush().snapshot_err("Flush failed")
    }

    pub fn write_entry(&mut self, entry: &Entry) -> Result<usize, Error> {
        let meta = serde_cbor::to_vec(entry).snapshot_err("Create metadata failed")?;
        let mut written: usize = 0;

        {
            let len = meta.len() as u32;
            let bytes = len.into_le_bytes();
            self.writer
                .write_all(&bytes)
                .snapshot_err("Write metadata length failed")?;
            written += bytes.len();
        };

        {
            let bytes = meta.as_slice();
            self.writer
                .write_all(&bytes)
                .snapshot_err("Write metadata bytes failed")?;
            written += bytes.len();
        };

        Stats::current().packing().inc(written);
        Ok(written)
    }

    pub fn write_file<P>(&mut self, file: &mut File, path: P, len: usize) -> Result<usize, Error>
    where
        P: AsRef<Path>,
    {
        Stats::current().packing().inc(len);

        if len == 0 {
            return Ok(0);
        }

        if len < MEM_MAP_THRESHOLD {
            let mut buf = vec![0u8; len];
            file.read_exact(&mut buf).io_err(path)?;
            self.writer
                .write_all(&buf)
                .snapshot_err("Write data failed")?;
        } else {
            let mut opts = memmap::MmapOptions::new();

            opts.len(len);
            let mapped = unsafe { opts.map(&file) };
            let mapped = mapped.io_err(&path)?;

            self.writer
                .write_all(mapped.as_ref())
                .snapshot_err("Write data failed")?;
        }

        Ok(len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs::File;

    use crate::errors::ResultExt;
    use crate::snapshot::Entry;
    use crate::testing::{self, B_FILE_PATH};

    #[test]
    fn write_file_entry() {
        let dst = testing::temp_file(".sn");
        let mut snapshot = Writing::open(&dst).unwrap();

        let file_entry = Entry::try_from_path(B_FILE_PATH).unwrap();
        assert_eq!(file_entry.as_file().is_some(), true);

        let written = snapshot.write_entry(&file_entry).unwrap();
        assert_eq!(written, 129);

        let (path, _, _, len) = file_entry.as_file().unwrap();
        let mut file = File::open(&path).io_err(&path).unwrap();
        let written = snapshot.write_file(&mut file, &path, len).unwrap();
        assert_eq!(written, 82944);

        snapshot.flush().unwrap();
    }
}
