use std::fs::{File, OpenOptions as FileOptions};
use std::io::{Seek, SeekFrom, Write};
use std::os::unix::fs::FileExt;
use std::path::Path;

use memmap::{Mmap, MmapMut, MmapOptions};

use crate::errors::ResultExt;
use crate::Error;

pub fn read<P>(path: P) -> Result<(File, Mmap), Error>
where
    P: AsRef<Path>,
{
    let file = File::open(&path).io_err(&path)?;
    let len = file.metadata().io_err(&path)?.len() as usize;
    let mut opts = MmapOptions::new();
    opts.len(len);

    let mmap = unsafe { opts.map(&file) };
    let mmap = mmap.io_err(&path)?;

    Ok((file, mmap))
}

pub fn write<P>(path: P, len: usize) -> Result<(File, MmapMut), Error>
where
    P: AsRef<Path>,
{
    let mut opts = FileOptions::new();
    let file = opts
        .create(true)
        .truncate(true)
        .read(true)
        .write(true)
        .open(&path)
        .io_err(&path)?;

    // Allocate space in the file first
    file.write_at(&[0], (len - 1) as u64).io_err(&path)?;

    let mut opts = MmapOptions::new();
    opts.len(len);

    let mmap = unsafe { opts.map_mut(&file) };
    let mmap = mmap.io_err(&path)?;

    Ok((file, mmap))
}
