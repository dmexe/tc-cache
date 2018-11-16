mod teamcity;

use self::teamcity::TeamCity;

pub trait Environment {
    fn name(&self) -> &str;
    fn project_id(&self) -> &str;
    fn is_default_branch(&self) -> bool;
    fn snapshot_url(&self) -> Option<&str>;
}
