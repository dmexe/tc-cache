use std::io::{Cursor, Write};
use std::str::FromStr;
use std::string::ToString;

use futures::stream::{iter_ok, Stream};
use futures::Future;
use rusoto_core::Region;
use rusoto_s3::{self as s3_api, S3Client, S3 as S3Api};
use url::{Host, Url};

use crate::errors::ResultExt;
use crate::storage::backend::{Backend, DownloadRequest, UploadRequest};
use crate::storage::futures_ext::FuturesExt;
use crate::{mmap, Error};

const S3_URI_SCHEME: &str = "s3";
const REGION_QUERY_KEY: &str = "region";
const ENDPOINT_QUERY_KEY: &str = "endpoint";
const CHUNK_SIZE: usize = 1024 * 1024 * 10; // 10mb
const CONCURRENCY: usize = 10;

#[derive(Debug)]
pub struct S3 {
    bucket_name: String,
    key_prefix: Option<String>,
    region: Region,
}

impl S3 {
    pub fn from(uri: &Url) -> Result<Self, Error> {
        let bucket_name = match uri.host() {
            Some(Host::Domain(host)) => host.to_string(),
            host => {
                let err = format!("Unrecognized bucket '{:?}'", host);
                return Err(Error::storage(err));
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
            .clone()
            .find(|it| it.0.as_ref() == REGION_QUERY_KEY)
            .map(|it| it.1.to_string());

        let endpoint = query
            .find(|it| it.0.as_ref() == ENDPOINT_QUERY_KEY)
            .map(|it| it.1.to_string());

        let region = match (default_region, endpoint) {
            (_, Some(endpoint)) => Region::Custom {
                name: "custom".into(),
                endpoint,
            },
            (Some(name), _) => {
                Region::from_str(name.as_str()).unwrap_or_else(|_| Region::default())
            }
            _ => Region::default(),
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

    fn key_prefixed<S>(&self, key: S) -> String
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

impl Backend for S3 {
    fn download(&self, req: DownloadRequest) -> Result<usize, Error> {
        let client = S3Client::new(self.region.clone());
        let path = &req.path.as_path();

        let get_object = s3_api::GetObjectRequest {
            bucket: self.bucket_name.clone(),
            key: self.key_prefixed(&req.key),
            ..Default::default()
        };

        let resp = client
            .get_object(get_object)
            .map_err(Error::storage)
            .sync()?;

        let body = resp.body.ok_or_else(|| Error::storage("body must be"))?;
        let content_len = resp
            .content_length
            .map(|it| it as usize)
            .ok_or_else(|| Error::storage("content length must be"))?;

        if content_len < 1 {
            let err = format!("Content length must be positive, got {}", content_len);
            return Err(Error::storage(err));
        }

        let (mut _file, mut dst) = mmap::write(path, content_len)?;
        let mut cursor = Cursor::new(dst.as_mut());

        body.map_err(Error::storage)
            .and_then(|chunk| cursor.write_all(&chunk).io_err(&path))
            .collect()
            .wait()?;

        Ok(content_len)
    }

    fn upload(&self, req: UploadRequest) -> Result<usize, Error> {
        let client = S3Client::new(self.region.clone());
        let key = self.key_prefixed(&req.key);

        let upload = s3_api::CreateMultipartUploadRequest {
            bucket: self.bucket_name.clone(),
            key: key.clone(),
            ..Default::default()
        };

        let upload = client
            .create_multipart_upload(upload)
            .map_err(Error::storage)
            .sync()?;

        let upload_id = upload
            .upload_id
            .ok_or_else(|| Error::storage("upload_id cannot be empty"))?;

        let (_, len, src) = mmap::read(&req.path, None)?;

        let parts = src
            .chunks(CHUNK_SIZE)
            .enumerate()
            .map(|(part_number, chunk)| {
                let part_number = (part_number + 1) as i64;
                let body = Vec::from(chunk);
                let part = s3_api::UploadPartRequest {
                    body: Some(body.into()),
                    bucket: self.bucket_name.clone(),
                    key: key.clone(),
                    upload_id: upload_id.clone(),
                    part_number: part_number as i64,
                    ..Default::default()
                };
                client
                    .upload_part(part)
                    .map(move |res| s3_api::CompletedPart {
                        e_tag: res.e_tag.clone(),
                        part_number: Some(part_number),
                    })
            })
            .collect::<Vec<_>>();

        let parts = iter_ok(parts)
            .buffered(CONCURRENCY)
            .collect()
            .map_err(Error::storage)
            .sync()?;

        let complete = s3_api::CompleteMultipartUploadRequest {
            bucket: self.bucket_name.clone(),
            key: key.clone(),
            upload_id,
            multipart_upload: Some(s3_api::CompletedMultipartUpload { parts: Some(parts) }),
            ..Default::default()
        };

        client
            .complete_multipart_upload(complete)
            .map_err(Error::storage)
            .sync()?;

        Ok(len)
    }
}

impl ToString for S3 {
    fn to_string(&self) -> String {
        let mut buf = format!("s3://{}", self.bucket_name);

        if let Some(prefix) = &self.key_prefix {
            buf = format!("{}/{}", buf, prefix);
        };

        match &self.region {
            Region::Custom {
                name: _name,
                endpoint,
            } => buf = format!("{}?endpoint={}", buf, endpoint),
            region => buf = format!("{}?region={}", buf, region.name()),
        }

        buf
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
            (
                "s3://bucket-name/?endpoint=http://localhost:8080",
                "s3://bucket-name?endpoint=http://localhost:8080"
            ),
        ];

        for (uri, expected) in params {
            let uri = Url::parse(uri).unwrap();
            let actual = S3::from(&uri).unwrap();
            assert!(
                actual.to_string().starts_with(expected),
                format!(
                    "Expect that '{}' starts with '{}'",
                    actual.to_string(),
                    expected
                )
            );
        }
    }

    #[test]
    fn upload() {
        let endpoint = match env::var("S3_ENDPOINT") {
            Ok(val) => val,
            Err(_) => return,
        };

        let uri = format!("s3://teamcity/cache?endpoint={}", endpoint);
        let uri = Url::parse(&uri).unwrap();
        let s3 = S3::from(&uri).unwrap();
        let len = { File::open(&B_FILE_PATH).unwrap().metadata().unwrap().len() as usize };
        let dst = temp_file(".s3");

        let upload = UploadRequest {
            path: B_FILE_PATH.into(),
            len,
            key: "file".into(),
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
