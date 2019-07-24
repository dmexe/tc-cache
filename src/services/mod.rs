use std::collections::HashMap;
use std::fmt::Display;
use std::path::Path;

mod generic;
mod teamcity;

use self::generic::Generic;
use self::teamcity::TeamCity;
use crate::Error;

pub trait Service: Display {
    fn project_id(&self) -> &str;
    fn is_uploadable(&self) -> bool;
    fn remote_url(&self) -> &str;
    fn into_box(self) -> Box<dyn Service>;
}

#[derive(Debug)]
pub struct ServiceFactory;

impl ServiceFactory {
    pub fn from_env<P>(
        env: &HashMap<String, String>,
        teamcity_build_properties_path: Option<P>,
    ) -> Result<Box<dyn Service>, Error>
    where
        P: AsRef<Path>,
    {
        if let Some(path) = teamcity_build_properties_path {
            let teamcity = TeamCity::from_path(env, path)?;
            return Ok(teamcity.into_box());
        }

        if Generic::is_available(&env) {
            return Generic::from_env(&env).map(Service::into_box);
        }

        if TeamCity::is_available(&env) {
            return TeamCity::from_env(&env).map(Service::into_box);
        }

        let err = format!("Unable to detect service");
        return Err(Error::unrecognized_service(err));
    }
}
