use std::collections::HashMap;
use std::fmt::{self, Display};
use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::errors::ResultExt;
use crate::{Error, Service};

const TEAMCITY_VERSION: &str = "teamcity.version";
const TEAMCITY_SERVER_URL: &str = "teamcity.serverUrl";
const TEAMCITY_PROJECT_ID: &str = "teamcity.project.id";
const TEAMCITY_BUILD_BRANCH_IS_DEFAULT: &str = "teamcity.build.branch.is_default";
const TC_CACHE_REMOTE_URL: &str = "tc.cache.remote.url";
const TEAMCITY_BUILD_PROPERTIES_FILE: &str = "TEAMCITY_BUILD_PROPERTIES_FILE";
const TEAMCITY_CONFIGURATION_PROPERTIES_FILE: &str = "teamcity.configuration.properties.file";

pub struct TeamCity {
    name: String,
    project_id: String,
    is_default_branch: bool,
    remote_url: String,
}

impl TeamCity {
    #[inline]
    pub fn is_available(env: &HashMap<String, String>) -> bool {
        env.contains_key(TEAMCITY_BUILD_PROPERTIES_FILE)
    }

    pub fn from_env(env: &HashMap<String, String>) -> Result<Self, Error> {
        let props_path = match env.get(TEAMCITY_BUILD_PROPERTIES_FILE) {
            Some(val) => val,
            None => {
                let err = format!(
                    "Environment variable '{}' wasn't found",
                    TEAMCITY_BUILD_PROPERTIES_FILE
                );
                return Err(Error::unrecognized_service(err));
            }
        };

        let props_path = Path::new(props_path);
        TeamCity::from_path(props_path)
    }

    pub fn from_path<P>(path: P) -> Result<Self, Error>
    where
        P: AsRef<Path>,
    {
        let props = Props::from_path(&path)?;
        let version = props.key(TEAMCITY_VERSION)?;
        let remote_url = props.key(TC_CACHE_REMOTE_URL).map(str::to_string)?;

        let config_path = props.key(TEAMCITY_CONFIGURATION_PROPERTIES_FILE)?;
        let props = Props::from_path(config_path)?;

        let server_url = props.key(TEAMCITY_SERVER_URL)?;
        let project_id = props.key(TEAMCITY_PROJECT_ID)?.to_string();

        let is_default_branch = props
            .key(TEAMCITY_BUILD_BRANCH_IS_DEFAULT)
            .map(|it| it == "true")
            .unwrap_or(false);

        let name = format!("{} at {}", version, server_url);

        Ok(TeamCity {
            name,
            project_id,
            is_default_branch,
            remote_url,
        })
    }
}

impl Display for TeamCity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "TeamCity {}", self.name)
    }
}

impl Service for TeamCity {
    #[inline]
    fn project_id(&self) -> &str {
        self.project_id.as_str()
    }

    #[inline]
    fn is_uploadable(&self) -> bool {
        self.is_default_branch
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

#[derive(Debug)]
struct Props(HashMap<String, String>);

impl Props {
    fn from_path<P>(path: P) -> Result<Self, Error>
    where
        P: AsRef<Path>,
    {
        if !path.as_ref().exists() {
            return Error::io_err(&path, "doesn't exists");
        }

        let mut file = File::open(&path).io_err(&path)?;
        let mut content = String::new();

        file.read_to_string(&mut content).io_err(&path)?;

        Ok(Props::from_content(content.as_str()))
    }

    fn from_content(content: &str) -> Self {
        let props = content
            .lines()
            .filter_map(Props::parse)
            .collect::<HashMap<String, String>>();

        Props(props)
    }

    fn key(&self, key: &str) -> Result<&str, Error> {
        let value = self
            .0
            .iter()
            .find(|it| it.0.as_str() == key)
            .map(|it| it.1.as_str());

        match value {
            Some(val) => Ok(val),
            None => {
                let err = format!("Cannot found key '{}' in build properties", key);
                Err(Error::unrecognized_service(err))
            }
        }
    }

    fn parse(line: &str) -> Option<(String, String)> {
        let line = line.trim();
        if line.starts_with('#') {
            return None;
        }

        if let Some(idx) = line.find('=') {
            let key = line[0..idx].trim().to_string();
            let value = line[(idx + 1)..].trim().to_string();
            let value = value.replace("\\:", ":").replace("\\=", "=");

            if key.is_empty() || value.is_empty() {
                return None;
            }

            return Some((key, value));
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::testing::{TEAMCITY_BUILD_PROPS_PATH, TEAMCITY_CONFIG_PROPS_PATH};

    #[test]
    fn parse_build_properties() {
        let props = Props::from_path(TEAMCITY_BUILD_PROPS_PATH).unwrap();

        assert_eq!(props.key("foo").ok(), None);

        assert_eq!(
            props.key(TEAMCITY_VERSION).unwrap(),
            "2018.1.3 (build 58658)",
            "{:?}",
            props
        );

        assert_eq!(
            props.key(TC_CACHE_REMOTE_URL).unwrap(),
            "s3://teamcity/cache?endpoint=http://127.0.0.1:9000",
            "{:?}",
            props
        );

        assert_eq!(
            props.key(TEAMCITY_CONFIGURATION_PROPERTIES_FILE).unwrap(),
            "tests/fixtures/teamcity/config.properties",
            "{:?}",
            props
        );
    }

    #[test]
    fn parse_configuration_properties() {
        let props = Props::from_path(TEAMCITY_CONFIG_PROPS_PATH).unwrap();

        assert_eq!(props.key("foo").ok(), None);

        assert_eq!(
            props.key(TEAMCITY_SERVER_URL).unwrap(),
            "http://localhost:8111",
            "{:?}",
            props
        );
        assert_eq!(
            props.key(TEAMCITY_PROJECT_ID).unwrap(),
            "Github_Example_Example",
            "{:?}",
            props
        );
        assert_eq!(
            props.key(TEAMCITY_BUILD_BRANCH_IS_DEFAULT).unwrap(),
            "true",
            "{:?}",
            props
        );
    }

    #[test]
    fn from_env() {
        let env = {
            let mut env = HashMap::new();
            env.insert(
                TEAMCITY_BUILD_PROPERTIES_FILE.into(),
                TEAMCITY_BUILD_PROPS_PATH.into(),
            );
            TeamCity::from_env(&env).unwrap()
        };

        assert_eq!(
            env.to_string(),
            "TeamCity 2018.1.3 (build 58658) at http://localhost:8111"
        );
        assert_eq!(env.project_id(), "Github_Example_Example");
        assert_eq!(env.is_uploadable(), true);
        assert_eq!(
            env.remote_url(),
            "s3://teamcity/cache?endpoint=http://127.0.0.1:9000"
        );
    }
}
