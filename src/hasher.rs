use std::fs::File;
use std::io::{Error as IoError, ErrorKind as IoErrorKind, Read};

use md5::Digest;

const MEM_MAP_THRESHOLD: usize = 64 * 1024; // 64k

pub fn md5(file: File) -> Result<String, IoError> {
    let len = file.metadata()?.len();
    md5_with_len(file, len as usize)
}

pub fn md5_with_len(mut file: File, len: usize) -> Result<String, IoError> {
    let hasher = md5::Md5::new();

    if len < MEM_MAP_THRESHOLD {
        hash_file(&mut file, hasher, len)
    } else {
        hash_mapped_file(&file, hasher, len)
    }
}

pub fn md5_bytes(src: &[u8]) -> String {
    let mut hasher = md5::Md5::new();
    hasher.input(src);
    let result = hasher.result();
    hex::encode(&result)
}

fn hash_file<D: Digest>(file: &mut File, mut hasher: D, len: usize) -> Result<String, IoError> {
    assert!(
        len < MEM_MAP_THRESHOLD,
        "file's len must be less then {}, got {}",
        MEM_MAP_THRESHOLD,
        len
    );

    let mut buf: [u8; MEM_MAP_THRESHOLD] = [0; MEM_MAP_THRESHOLD];
    file.read_exact(&mut buf[0..len])?;

    hasher.input(&buf[0..len]);
    let result = hasher.result();

    Ok(hex::encode(&result))
}

fn hash_mapped_file<D: Digest>(file: &File, mut hasher: D, len: usize) -> Result<String, IoError> {
    let mut opts = memmap::MmapOptions::new();
    opts.len(len as usize);

    let mapped = unsafe { opts.map(file) };
    let mapped = mapped.map_err(|err| IoError::new(IoErrorKind::Other, err))?;

    hasher.input(mapped);

    Ok(hex::encode(&hasher.result()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{A_FILE_PATH, B_FILE_PATH};

    #[test]
    fn md5_for_regular_file() {
        let hash = File::open(A_FILE_PATH).and_then(md5).unwrap();
        assert_eq!(hash, "0cc175b9c0f1b6a831c399e269772661")
    }

    #[test]
    fn md5_for_mapped_file() {
        let hash = File::open(B_FILE_PATH).and_then(md5).unwrap();
        assert_eq!(hash, "54510be579370aa078fbb9c5d9eed53a")
    }
}
