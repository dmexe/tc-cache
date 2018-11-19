use std::fmt::{self, Display};
use std::str::FromStr;
use std::string::ToString;

use rusoto_core::Region;
use url::{Host, Url};

use crate::Error;

const S3_URI_SCHEME: &str = "s3";
const REGION_QUERY_KEY: &str = "region";

#[derive(Debug)]
pub struct S3 {
    bucket_name: String,
    key_prefix: Option<String>,
    region: Region,
}

impl S3 {
    pub fn parse<S>(uri: S) -> Result<Self, Error>
    where
        S: ToString,
    {
        let uri = Url::parse(uri.to_string().as_str()).unwrap();
        match uri.scheme() {
            S3_URI_SCHEME => {}
            scheme @ _ => {
                let err = format!("Unknown scheme '{}'", scheme);
                return Err(Error::unrecognized_snapshot_url(err));
            }
        };

        let bucket_name = match uri.host() {
            Some(Host::Domain(host)) => host.to_string(),
            host @ _ => {
                let err = format!("Unrecognized bucket '{:?}'", host);
                return Err(Error::unrecognized_snapshot_url(err));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ok() {
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
            let actual = S3::parse(uri).unwrap();
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
            match S3::parse(uri) {
                Err(_) => {}
                Ok(ok) => unreachable!("{:?}", ok),
            }
        }
    }
}
