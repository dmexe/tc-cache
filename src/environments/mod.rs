use std::fmt::Display;

mod teamcity_env;

pub use self::teamcity_env::TeamCityEnv;

pub trait Environment: Display {
    fn project_id(&self) -> &str;
    fn is_uploadable(&self) -> bool;
    fn snapshot_url(&self) -> &str;
}
