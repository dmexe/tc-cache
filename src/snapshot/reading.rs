use std::fs::OpenOptions;
use std::io::ErrorKind::UnexpectedEof;
use std::io::{Cursor, Error as IoError, Read, Write};
use std::path::Path;

use memmap::{Mmap, MmapOptions};

use crate::bytes::FromLeBytes;
use crate::errors::ResultExt;
use crate::snapshot::{Entry, BUFFER_SIZE, VERSION, VERSION_LEN};
use crate::{Error, Stats};

#[derive(Debug)]
pub struct Reading<R = ()> {
    reader: R,
}

impl Reading {
    pub fn from<R: Read>(reader: R) -> Result<Reading<snap::Reader<R>>, Error> {
        let mut reader = Reading {
            reader: snap::Reader::new(reader),
        };

        reader.check_version()?;
        Ok(reader)
    }

    pub fn open<P: AsRef<Path>>(path: P) -> Result<Reading<snap::Reader<Cursor<Mmap>>>, Error> {
        let file = OpenOptions::new().read(true).open(&path).io_err(&path)?;

        let opts = MmapOptions::new();
        let mapped = unsafe { opts.map(&file) };
        let mapped = mapped.io_err(&path)?;

        Reading::from(Cursor::new(mapped))
    }
}

impl<R: Read> Reading<R> {
    fn check_version(&mut self) -> Result<(), Error> {
        Stats::current().unpacking().inc(VERSION_LEN);

        let src = &mut self.reader;
        let mut buf: [u8; VERSION_LEN] = [0; VERSION_LEN];

        src.read_exact(&mut buf)
            .snapshot_err("Read version header failed")?;

        if VERSION != &buf {
            let err = format!("Expected {:?}, got {:?}", VERSION, buf);
            Error::snapshot_err("Version header mismatch", err)
        } else {
            Ok(())
        }
    }
    pub fn read_entry(&mut self) -> Result<Option<(Entry, usize)>, Error> {
        let src = &mut self.reader;
        let mut buf: [u8; 4] = [0; 4];

        if let Err(err) = src.read_exact(&mut buf) {
            if err.kind() == UnexpectedEof {
                return Ok(None);
            } else {
                return Err(Error::snapshot("Read entry size failed")(err));
            }
        }
        let len = u32::from_le_bytes(buf) as usize;
        let mut buf = vec![0u8; len];

        src.read_exact(&mut buf).snapshot_err("Read entry failed")?;

        let entry = serde_cbor::from_slice(&buf).snapshot_err("Read entry failed")?;
        let len = buf.len() + 4;

        Stats::current().unpacking().inc(len);

        Ok(Some((entry, len)))
    }

    pub fn copy_to<W: Write>(&mut self, dst: &mut W, mut len: usize) -> Result<usize, Error> {
        Stats::current().unpacking().inc(len);

        let src = &mut self.reader;
        let mut buf: [u8; BUFFER_SIZE] = [0; BUFFER_SIZE];
        let mut written: usize = 0;

        loop {
            if len == 0 {
                return Ok(written);
            }

            let chunk = BUFFER_SIZE.min(len);

            src.read_exact(&mut buf[..chunk])
                .snapshot_err("Copy failed")?;
            dst.write_all(&buf[..chunk]).snapshot_err("Copy failed")?;

            written += chunk;
            len -= chunk;
        }
    }

    pub fn skip(&mut self, len: usize) -> Result<usize, Error> {
        Stats::current().unpacking().inc(len);

        let mut null = Null;
        self.copy_to(&mut null, len)
    }
}

struct Null;

impl Write for Null {
    fn write(&mut self, buf: &[u8]) -> Result<usize, IoError> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<(), IoError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs::File;
    use std::path::Path;

    use crate::errors::ResultExt;
    use crate::hashing;
    use crate::snapshot::{Entry, Writing};
    use crate::testing::{self, B_FILE_PATH};

    #[test]
    fn read_file_entry() {
        let dst = testing::temp_file(".sn");

        {
            let mut snapshot = Writing::open(&dst).unwrap();

            let file_entry = Entry::try_from_path(B_FILE_PATH).unwrap();
            snapshot.write_entry(&file_entry).unwrap();

            let (path, _, _, len) = file_entry.as_file().unwrap();
            let mut file = File::open(&path).io_err(&path).unwrap();

            snapshot.write_file(&mut file, &path, len).unwrap();
            snapshot.flush().unwrap();
        }

        {
            let mut snapshot = Reading::open(&dst).unwrap();
            let (file_entry, _) = snapshot.read_entry().unwrap().unwrap();
            let (path, _, md5, len) = file_entry.as_file().unwrap();

            assert_eq!(path, Path::new(B_FILE_PATH));
            assert_eq!(md5, "54510be579370aa078fbb9c5d9eed53a");
            assert_eq!(len, 82944);

            let mut buf = Vec::new();
            snapshot.copy_to(&mut buf, len).unwrap();

            let actual = hashing::md5::bytes(&buf);
            assert_eq!(md5, actual);

            assert_eq!(snapshot.read_entry().unwrap().is_none(), true);
        }
    }
}
