use std::convert::TryFrom;
use std::fmt::{self, Display};
use std::io::{Cursor, Write};
use std::str::FromStr;
use std::string::ToString;

use futures::stream::{iter_ok, Stream};
use futures::sync::oneshot::spawn;
use futures::Future;
use rusoto_core::Region;
use rusoto_s3::{
    CompleteMultipartUploadRequest, CompletedMultipartUpload, CompletedPart,
    CreateMultipartUploadRequest, GetObjectRequest, UploadPartRequest,
};
use rusoto_s3::{S3Client, S3 as S3Api};
use tokio::runtime::Runtime;
use url::{Host, Url};

use crate::errors::ResultExt;
use crate::remote::{DownloadRequest, UploadRequest};
use crate::{mmap, Error, Remote};

const S3_URI_SCHEME: &str = "s3";
const REGION_QUERY_KEY: &str = "region";
const CHUNK_SIZE: usize = 1024 * 1024 * 10; // 1mb
const CONCURRENCY: usize = 10;

#[derive(Debug)]
pub struct S3 {
    bucket_name: String,
    key_prefix: Option<String>,
    region: Region,
}

impl S3 {
    pub fn from(uri: &Url) -> Result<Self, Error> {
        match uri.scheme() {
            S3_URI_SCHEME => {}
            scheme @ _ => {
                let err = format!("Unknown scheme '{}'", scheme);
                return Err(Error::remote(err));
            }
        };

        let bucket_name = match uri.host() {
            Some(Host::Domain(host)) => host.to_string(),
            host @ _ => {
                let err = format!("Unrecognized bucket '{:?}'", host);
                return Err(Error::remote(err));
            }
        };

        let mut key_prefix = uri.path().to_string();
        if key_prefix.starts_with('/') {
            key_prefix = key_prefix.drain(1..).collect()
        };

        let key_prefix = if key_prefix.is_empty() {
            None
        } else {
            Some(key_prefix.to_string())
        };

        let mut query = uri.query_pairs();
        let default_region = query
            .find(|it| it.0.as_ref() == REGION_QUERY_KEY)
            .map(|it| it.1.to_string());

        let region = match default_region {
            Some(name) => Region::from_str(name.as_str()).unwrap_or_else(|_| Region::default()),
            None => Region::default(),
        };

        let s3 = S3 {
            bucket_name,
            key_prefix,
            region,
        };

        Ok(s3)
    }

    pub fn scheme() -> &'static str {
        S3_URI_SCHEME
    }

    pub fn endpoint<S>(self, endpoint: S) -> Self
    where
        S: Into<String>,
    {
        let region = Region::Custom {
            name: self.region.name().to_string(),
            endpoint: endpoint.into(),
        };

        Self { region, ..self }
    }

    fn prefixed<S>(&self, key: S) -> String
    where
        S: AsRef<str>,
    {
        if let Some(val) = &self.key_prefix {
            format!("{}/{}", val, key.as_ref())
        } else {
            key.as_ref().to_string()
        }
    }
}

impl Display for S3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{}://", S3_URI_SCHEME)?;
        write!(f, "{}", self.bucket_name)?;

        if let Some(ref prefix) = self.key_prefix {
            write!(f, "/{}", prefix)?;
        }

        write!(f, "?{}={}", REGION_QUERY_KEY, self.region.name())
    }
}

impl Remote for S3 {
    fn key(self, key: &str) -> S3 {
        let key = match &self.key_prefix {
            Some(val) => format!("{}/{}", val, key),
            None => key.to_string(),
        };

        S3 {
            key_prefix: Some(key),
            ..self
        }
    }

    fn download(&self, req: DownloadRequest) -> Result<(), Error> {
        let client = S3Client::new(self.region.clone());
        let path = &req.path.as_path();

        let get_object = GetObjectRequest {
            bucket: self.bucket_name.clone(),
            key: self.prefixed(&req.key),
            ..Default::default()
        };

        let resp = client
            .get_object(get_object)
            .sync()
            .map_err(Error::remote)?;
        let body = resp.body.ok_or_else(|| Error::remote("body must be"))?;
        let content_len = resp
            .content_length
            .ok_or_else(|| Error::remote("content length must be"))?;
        let content_len = content_len as usize;

        if content_len < 1 {
            let err = format!("Content length must be positive, got {}", content_len);
            return Err(Error::remote(err));
        }

        let (mut _file, mut dst) = mmap::write(path, content_len)?;
        let mut cursor = Cursor::new(dst.as_mut());

        body.map_err(Error::remote)
            .and_then(|chunk| cursor.write_all(&chunk).io_err(&path))
            .collect()
            .wait()?;

        Ok(())
    }

    fn upload(&self, req: UploadRequest) -> Result<(), Error> {
        let runtime = Runtime::new().unwrap();
        let handle = runtime.executor();

        let client = S3Client::new(self.region.clone());
        let key = self.prefixed(&req.key);

        let upload = CreateMultipartUploadRequest {
            bucket: self.bucket_name.clone(),
            key: key.clone(),
            ..Default::default()
        };

        let upload = client
            .create_multipart_upload(upload)
            .map_err(Error::remote);

        let upload = spawn(upload, &handle).wait()?;

        let upload_id = upload
            .upload_id
            .ok_or_else(|| Error::remote("upload_id cannot be empty"))?;

        let (_, _, src) = mmap::read(&req.path, None)?;

        let parts = src
            .chunks(CHUNK_SIZE)
            .enumerate()
            .map(|(part_number, chunk)| {
                let part_number = (part_number + 1) as i64;
                let body = Vec::from(chunk);
                let part = UploadPartRequest {
                    body: Some(body.into()),
                    bucket: self.bucket_name.clone(),
                    key: key.clone(),
                    upload_id: upload_id.clone(),
                    part_number: part_number as i64,
                    ..Default::default()
                };
                client.upload_part(part).map(move |res| CompletedPart {
                    e_tag: res.e_tag.clone(),
                    part_number: Some(part_number),
                })
            })
            .collect::<Vec<_>>();

        let parts = iter_ok(parts)
            .buffered(CONCURRENCY)
            .collect()
            .map_err(Error::remote);

        let parts = spawn(parts, &handle).wait()?;

        let complete = CompleteMultipartUploadRequest {
            bucket: self.bucket_name.clone(),
            key: key.clone(),
            upload_id,
            multipart_upload: Some(CompletedMultipartUpload { parts: Some(parts) }),
            ..Default::default()
        };

        let complete = client
            .complete_multipart_upload(complete)
            .map_err(Error::remote);

        spawn(complete, &handle).wait()?;

        Ok(())
    }
}

impl TryFrom<&str> for S3 {
    type Error = Error;

    fn try_from(uri: &str) -> Result<Self, Self::Error> {
        let uri = Url::parse(uri).map_err(Error::remote)?;
        if uri.scheme() == S3::scheme() {
            return S3::from(&uri);
        }

        let err = format!("Unknown remote uri '{}'", uri);
        Err(Error::remote(err))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::env;
    use std::fs::File;

    use crate::hashing;
    use crate::testing::{temp_file, B_FILE_PATH};

    #[test]
    fn from_url() {
        #[rustfmt::skip]
        let params = vec![
            (
                "s3://bucket-name/prefix?region=eu-west-1",
                "s3://bucket-name/prefix?region=eu-west-1",
            ),
            (
                "s3://bucket-name?region=eu-west-1",
                "s3://bucket-name?region=eu-west-1",
            ),
            (
                "s3://bucket-name/prefix",
                "s3://bucket-name/prefix?region="
            ),
            (
                "s3://bucket-name", 
                "s3://bucket-name?region="
            ),
        ];

        for (uri, expected) in params {
            let uri = Url::parse(uri).unwrap();
            let actual = S3::from(&uri).unwrap();
            assert!(
                actual.to_string().starts_with(expected),
                format!("Expect that '{}' starts with '{}'", actual, expected)
            );
        }
    }

    #[test]
    fn parse_err() {
        #[rustfmt::skip]
        let params = vec! { 
            "http://example.com" 
        };

        for uri in params {
            let uri = Url::parse(uri).unwrap();
            match S3::from(&uri) {
                Err(_) => {}
                Ok(ok) => unreachable!("{:?}", ok),
            }
        }
    }

    #[test]
    fn upload() {
        env::set_var("AWS_ACCESS_KEY_ID", "accessKey");
        env::set_var("AWS_SECRET_ACCESS_KEY", "secretKey");

        let uri = Url::parse("s3://bucket/prefix").unwrap();
        let s3 = S3::from(&uri).unwrap().endpoint("http://127.0.0.1:9000");
        let s3 = s3.key("projectId");
        let len = { File::open(&B_FILE_PATH).unwrap().metadata().unwrap().len() as usize };
        let dst = temp_file(".s3");

        let upload = UploadRequest {
            path: B_FILE_PATH.into(),
            len,
            key: "file".into(),
            ..Default::default()
        };

        s3.upload(upload).unwrap();

        let download = DownloadRequest {
            path: dst.as_ref().to_path_buf(),
            key: "file".into(),
        };

        s3.download(download).unwrap();

        let expected = hashing::md5::path(&B_FILE_PATH).unwrap();
        let actual = hashing::md5::path(&dst).unwrap();

        assert_eq!(expected, actual);
    }
}
