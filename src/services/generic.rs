use std::collections::HashMap;
use std::fmt::{self, Display};

const PROJECT_ID: &str = "TC_CACHE_PROJECT_ID";
const UPLOAD: &str = "TC_CACHE_UPLOAD";
const REMOTE_URL: &str = "TC_CACHE_REMOTE_URL";

use crate::services::Service;
use crate::Error;

#[derive(Debug)]
pub struct Generic {
    project_id: String,
    upload: bool,
    remote_url: String,
}

impl Generic {
    #[inline]
    pub fn is_available(env: &HashMap<String, String>) -> bool {
        env.contains_key(PROJECT_ID) && env.contains_key(UPLOAD) && env.contains_key(REMOTE_URL)
    }

    pub fn from_env(env: &HashMap<String, String>) -> Result<Self, Error> {
        let project_id = match env.get(PROJECT_ID) {
            Some(ok) => ok.to_string(),
            None => {
                let err = format!("Environment variable '{}' was not found", PROJECT_ID);
                return Err(Error::unrecognized_service(err));
            }
        };

        let upload = match env.get(UPLOAD) {
            Some(val) => val == "1" || val == "true",
            None => {
                let err = format!("Environment variable '{}' was not found", UPLOAD);
                return Err(Error::unrecognized_service(err));
            }
        };

        let remote_url = match env.get(REMOTE_URL) {
            Some(ok) => ok.to_string(),
            None => {
                let err = format!("Environment variable '{}' was not found", REMOTE_URL);
                return Err(Error::unrecognized_service(err));
            }
        };

        Ok(Generic {
            project_id,
            upload,
            remote_url,
        })
    }
}

impl Display for Generic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(
            f,
            "Env(project={}, upload={}, remote_url={})",
            self.project_id, self.upload, self.remote_url
        )
    }
}

impl Service for Generic {
    #[inline]
    fn project_id(&self) -> &str {
        self.project_id.as_str()
    }

    #[inline]
    fn is_uploadable(&self) -> bool {
        self.upload
    }

    #[inline]
    fn remote_url(&self) -> &str {
        self.remote_url.as_str()
    }

    #[inline]
    fn into_box(self) -> Box<dyn Service> {
        Box::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_env() {
        let mut env = HashMap::new();

        assert_eq!(Generic::is_available(&env), false);

        match Generic::from_env(&env) {
            Err(err) => assert!(err.to_string().contains("was not found")),
            Ok(ok) => unreachable!("{:?}", ok),
        }

        env.insert(PROJECT_ID.into(), "projectId".into());
        env.insert(UPLOAD.into(), "1".into());
        env.insert(REMOTE_URL.into(), "http://example.com".into());

        assert_eq!(Generic::is_available(&env), true);

        let generic = match Generic::from_env(&env) {
            Ok(ok) => ok,
            Err(err) => unreachable!("{:?}", err),
        };

        assert_eq!(
            generic.to_string(),
            "Env(project=projectId, upload=true, remote_url=http://example.com)"
        );
    }
}
