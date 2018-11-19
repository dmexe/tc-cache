use std::error::Error as StdError;
use std::fmt::{self, Display};
use std::path::{Path, PathBuf};

type Cause = Box<dyn StdError + Send + Sync + 'static>;

#[derive(Debug)]
pub enum ErrorKind {
    Io(PathBuf),
    Snapshot(String),
    UnrecognizedEnvironment,
    UnrecognizedSnapshotUrl,
}

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    cause: Option<Cause>,
}

impl Error {
    pub fn unrecognized_snapshot_url<E>(err: E) -> Error
    where
        E: Into<Cause>,
    {
        Error {
            kind: ErrorKind::UnrecognizedSnapshotUrl,
            cause: Some(err.into()),
        }
    }

    pub fn unrecognized_environment<E>(err: E) -> Error
    where
        E: Into<Cause>,
    {
        Error {
            kind: ErrorKind::UnrecognizedEnvironment,
            cause: Some(err.into()),
        }
    }

    pub fn io<T, E>(path: T) -> impl FnOnce(E) -> Error
    where
        T: AsRef<Path>,
        E: Into<Cause>,
    {
        let path = path.as_ref().to_path_buf();
        |err: E| Error {
            kind: ErrorKind::Io(path),
            cause: Some(err.into()),
        }
    }

    pub fn snapshot<T, E>(message: T) -> impl FnOnce(E) -> Error
    where
        T: Into<String>,
        E: Into<Cause>,
    {
        |err: E| Error {
            kind: ErrorKind::Snapshot(message.into()),
            cause: Some(err.into()),
        }
    }

    pub fn io_err<T, R, E>(path: T, err: E) -> Result<R, Error>
    where
        T: AsRef<Path>,
        E: Into<Cause>,
    {
        Err(Error {
            kind: ErrorKind::Io(path.as_ref().to_path_buf()),
            cause: Some(err.into()),
        })
    }

    pub fn snapshot_err<T, R, E>(message: T, err: E) -> Result<R, Error>
    where
        T: Into<String>,
        E: Into<Cause>,
    {
        Err(Error {
            kind: ErrorKind::Snapshot(message.into()),
            cause: Some(err.into()),
        })
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        match &self.kind {
            ErrorKind::Io(path) => write!(f, "{} at {:?}", self.description(), path.as_os_str())?,
            ErrorKind::Snapshot(message) => write!(f, "{}; {}", self.description(), message)?,
            ErrorKind::UnrecognizedEnvironment => write!(f, "{}", self.description())?,
            ErrorKind::UnrecognizedSnapshotUrl => write!(f, "{}", self.description())?,
        };

        let mut cause = self.source();
        while let Some(ref err) = cause {
            write!(f, "; {}", err)?;
            cause = err.source()
        }

        Ok(())
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        match &self.kind {
            ErrorKind::Io(_) => "I/O error",
            ErrorKind::Snapshot(_) => "Snapshot error",
            ErrorKind::UnrecognizedEnvironment => "Unrecognized environment",
            ErrorKind::UnrecognizedSnapshotUrl => "Unrecognized snapshot url",
        }
    }

    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        if let Some(ref err) = self.cause {
            return Some(err.as_ref());
        }
        None
    }
}

pub trait ResultExt<T, E> {
    fn io_err<P>(self, path: P) -> Result<T, Error>
    where
        P: AsRef<Path>;

    fn snapshot_err<S>(self, message: S) -> Result<T, Error>
    where
        S: Into<String>;
}

impl<T, E> ResultExt<T, E> for Result<T, E>
where
    E: Into<Cause>,
{
    fn io_err<P>(self, path: P) -> Result<T, Error>
    where
        P: AsRef<Path>,
    {
        self.map_err(Error::io(path))
    }

    fn snapshot_err<S>(self, message: S) -> Result<T, Error>
    where
        S: Into<String>,
    {
        self.map_err(Error::snapshot(message))
    }
}
