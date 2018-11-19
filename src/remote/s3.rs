use std::fmt::{self, Display};
use std::string::ToString;

use crate::Error;
use url::{Host, Url};

const S3_URI_SCHEME: &str = "s3";
const REGION_QUERY_KEY: &str = "region";

#[derive(Debug)]
pub struct S3 {
    bucket_name: String,
    prefix: Option<String>,
    default_region: Option<String>,
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

        let mut prefix = uri.path().to_string();
        if prefix.starts_with('/') {
            prefix = prefix.drain(1..).collect()
        };

        let prefix = if prefix.is_empty() {
            None
        } else {
            Some(prefix.to_string())
        };

        let mut query = uri.query_pairs();
        let default_region = query
            .find(|it| it.0.as_ref() == REGION_QUERY_KEY)
            .map(|it| it.1.to_string());

        let s3 = S3 {
            bucket_name,
            prefix,
            default_region,
        };

        Ok(s3)
    }
}

impl Display for S3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "{}://", S3_URI_SCHEME)?;
        write!(f, "{}", self.bucket_name)?;

        if let Some(ref prefix) = self.prefix {
            write!(f, "/{}", prefix)?;
        }

        if let Some(ref region) = self.default_region {
            write!(f, "?{}={}", REGION_QUERY_KEY, region)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let ok_params = vec![
            (
                "s3://bucket-name/prefix?region=eu-west",
                "s3://bucket-name/prefix?region=eu-west",
            ),
            (
                "s3://bucket-name?region=eu-west",
                "s3://bucket-name?region=eu-west",
            ),
            ("s3://bucket-name/prefix", "s3://bucket-name/prefix"),
            ("s3://bucket-name", "s3://bucket-name"),
        ];

        let err_params = vec!["http://example.com"];

        for (uri, expected) in ok_params {
            let actual = S3::parse(uri).unwrap();
            assert_eq!(actual.to_string(), expected.to_string());
        }

        for uri in err_params {
            match S3::parse(uri) {
                Err(_) => {}
                Ok(ok) => unreachable!("{:?}", ok),
            }
        }
    }
}
