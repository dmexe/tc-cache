use std::collections::HashMap;
use std::fmt::{self, Display};
use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::errors::ResultExt;
use crate::{Error, Service};

const TEAMCITY_VERSION_PROP: &str = "env.TEAMCITY_VERSION";
const TEAMCITY_SERVER_URL_PROP: &str = "teamcity.serverUrl";
const PROJECT_ID_PROP: &str = "teamcity.project.id";
const BUILD_BRANCH_IS_DEFAULT_PROP: &str = "teamcity.build.branch.is_default";
const CACHE_REMOTE_URL_PROP: &str = "tc.cache.remote.url";
const TEAMCITY_BUILD_PROPERTIES_FILE_ENV: &str = "TEAMCITY_BUILD_PROPERTIES_FILE";

pub struct TeamCity {
    name: String,
    project_id: String,
    is_default_branch: bool,
    remote_url: String,
}

impl TeamCity {
    pub fn is_available(env: &HashMap<String, String>) -> bool {
        env.contains_key(TEAMCITY_BUILD_PROPERTIES_FILE_ENV)
    }

    pub fn from_env(env: &HashMap<String, String>) -> Result<Self, Error> {
        let props_path = match env.get(TEAMCITY_BUILD_PROPERTIES_FILE_ENV) {
            Some(val) => val,
            None => {
                let err = format!(
                    "Environment variable '{}' wasn't found",
                    TEAMCITY_BUILD_PROPERTIES_FILE_ENV
                );
                return Err(Error::unrecognized_service(err));
            }
        };

        let props_path = Path::new(props_path);
        TeamCity::from_path(props_path)
    }

    pub fn from_path<P>(props_file_path: P) -> Result<Self, Error>
    where
        P: AsRef<Path>,
    {
        if !props_file_path.as_ref().exists() {
            return Error::io_err(&props_file_path, "doesn't exists");
        }

        let mut file = File::open(&props_file_path).io_err(&props_file_path)?;
        let mut content = String::new();

        file.read_to_string(&mut content).io_err(&props_file_path)?;

        TeamCity::from_props(content.as_str())
    }

    pub fn from_props(props: &str) -> Result<Self, Error> {
        let props = Props::from(props);
        let version = props.key(TEAMCITY_VERSION_PROP)?;
        let server_url = props.key(TEAMCITY_SERVER_URL_PROP)?;
        let project_id = props.key(PROJECT_ID_PROP)?.to_string();
        let remote_url = props.key(CACHE_REMOTE_URL_PROP).map(str::to_string)?;

        let is_default_branch = props
            .key(BUILD_BRANCH_IS_DEFAULT_PROP)
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
    fn project_id(&self) -> &str {
        self.project_id.as_str()
    }

    fn is_uploadable(&self) -> bool {
        self.is_default_branch
    }

    fn remote_url(&self) -> &str {
        self.remote_url.as_str()
    }

    fn into_box(self) -> Box<dyn Service> {
        Box::new(self)
    }
}

#[derive(Debug)]
struct Props(HashMap<String, String>);

impl Props {
    fn from(content: &str) -> Self {
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
            let value = value.replace("\\:", ":");

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

    use crate::testing::TEAMCITY_BUILD_PROPS_PATH;

    #[test]
    fn parse_props_file() {
        let props = {
            let mut file = File::open(TEAMCITY_BUILD_PROPS_PATH).unwrap();
            let mut content = String::new();

            file.read_to_string(&mut content).unwrap();
            Props::from(content.as_str())
        };

        assert_eq!(
            props.key(TEAMCITY_VERSION_PROP).unwrap(),
            "2018.1.3 (build 58658)",
            "{:?}",
            props
        );
        assert_eq!(
            props.key(TEAMCITY_SERVER_URL_PROP).unwrap(),
            "http://localhost:8111",
            "{:?}",
            props
        );
        assert_eq!(
            props.key(PROJECT_ID_PROP).unwrap(),
            "Github_Example_Example",
            "{:?}",
            props
        );
        assert_eq!(
            props.key(BUILD_BRANCH_IS_DEFAULT_PROP).unwrap(),
            "true",
            "{:?}",
            props
        );

        assert_eq!(props.key("foo").ok(), None);
    }

    #[test]
    fn from_env() {
        let env = {
            let mut env = HashMap::new();
            env.insert(
                TEAMCITY_BUILD_PROPERTIES_FILE_ENV.into(),
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
        assert_eq!(env.remote_url(), "s3://bucket/prefix");
    }
}
