use std::fmt::Display;

mod teamcity;

pub use self::teamcity::TeamCity;

pub trait Service: Display {
    fn project_id(&self) -> &str;
    fn is_uploadable(&self) -> bool;
    fn remote_url(&self) -> &str;
    fn into_box(self) -> Box<dyn Service>;
}
