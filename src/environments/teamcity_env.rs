use std::collections::HashMap;
use std::env;
use std::fmt::{self, Display};
use std::fs::File;
use std::io::Read;
use std::path::Path;

use log::warn;

use crate::errors::ResultExt;
use crate::Environment;

const TEAMCITY_VERSION_PROP: &str = "env.TEAMCITY_VERSION";
const TEAMCITY_SERVER_URL_PROP: &str = "teamcity.serverUrl";
const PROJECT_ID_PROP: &str = "teamcity.project.id";
const BUILD_BRANCH_IS_DEFAULT_PROP: &str = "teamcity.build.branch.is_default";
const CACHE_SNAPSHOT_URL_PROP: &str = "tc.cache.snapshot.url";
const TEAMCITY_BUILD_PROPERTIES_FILE_ENV: &str = "TEAMCITY_BUILD_PROPERTIES_FILE";

pub struct TeamCityEnv {
    name: String,
    project_id: String,
    is_default_branch: bool,
    snapshot_url: String,
}

impl TeamCityEnv {
    pub fn from_env() -> Option<Self> {
        let props_path = match env::var(TEAMCITY_BUILD_PROPERTIES_FILE_ENV) {
            Ok(ok) => ok,
            Err(_) => return None,
        };

        let props_path = Path::new(props_path.as_str());
        TeamCityEnv::from_path(props_path)
    }

    pub fn from_path<P>(props_file_path: P) -> Option<Self>
    where
        P: AsRef<Path>,
    {
        if !props_file_path.as_ref().exists() {
            return None;
        }

        let mut file = File::open(&props_file_path).io_err(&props_file_path).ok()?;
        let mut content = String::new();
        file.read_to_string(&mut content)
            .io_err(&props_file_path)
            .ok()?;
        TeamCityEnv::from_props(content.as_str())
    }

    pub fn from_props(props: &str) -> Option<Self> {
        let props = Props::from(props);
        let version = props.key(TEAMCITY_VERSION_PROP)?;
        let server_url = props.key(TEAMCITY_SERVER_URL_PROP)?;
        let project_id = props.key(PROJECT_ID_PROP)?.to_string();
        let snapshot_url = props.key(CACHE_SNAPSHOT_URL_PROP).map(str::to_string)?;
        let is_default_branch = props
            .key(BUILD_BRANCH_IS_DEFAULT_PROP)
            .map(|it| it == "true")
            .unwrap_or(false);
        let name = format!("{} at {}", version, server_url);

        Some(TeamCityEnv {
            name,
            project_id,
            is_default_branch,
            snapshot_url,
        })
    }

    pub fn into_box(self) -> Box<dyn Environment> {
        Box::new(self)
    }
}

impl Display for TeamCityEnv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        write!(f, "TeamCity {}", self.name)
    }
}

impl Environment for TeamCityEnv {
    fn project_id(&self) -> &str {
        self.project_id.as_str()
    }

    fn is_uploadable(&self) -> bool {
        self.is_default_branch
    }

    fn snapshot_url(&self) -> &str {
        self.snapshot_url.as_str()
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

    fn key(&self, key: &str) -> Option<&str> {
        let value = self
            .0
            .iter()
            .find(|it| it.0.as_str() == key)
            .map(|it| it.1.as_str());

        if value.is_none() {
            warn!("Cannot found key '{}' in build properties", key);
        };

        value
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
    fn read_props() {
        let props = {
            let mut file = File::open(TEAMCITY_BUILD_PROPS_PATH).unwrap();
            let mut content = String::new();

            file.read_to_string(&mut content).unwrap();
            Props::from(content.as_str())
        };

        assert_eq!(
            props.key(TEAMCITY_VERSION_PROP),
            Some("2018.1.3 (build 58658)"),
            "{:?}",
            props
        );
        assert_eq!(
            props.key(TEAMCITY_SERVER_URL_PROP),
            Some("http://localhost:8111"),
            "{:?}",
            props
        );
        assert_eq!(
            props.key(PROJECT_ID_PROP),
            Some("Github_Example_Example"),
            "{:?}",
            props
        );
        assert_eq!(
            props.key(BUILD_BRANCH_IS_DEFAULT_PROP),
            Some("true"),
            "{:?}",
            props
        );

        assert_eq!(props.key("foo"), None);
    }

    #[test]
    fn create_env() {
        let env = {
            let mut file = File::open(TEAMCITY_BUILD_PROPS_PATH).unwrap();
            let mut content = String::new();

            file.read_to_string(&mut content).unwrap();
            TeamCityEnv::from_props(content.as_str()).unwrap()
        };

        assert_eq!(
            env.to_string(),
            "2018.1.3 (build 58658) at http://localhost:8111"
        );
        assert_eq!(env.project_id(), "Github_Example_Example");
        assert_eq!(env.is_uploadable(), true);
        assert_eq!(env.snapshot_url(), "s3://bucket/prefix");
    }
}
