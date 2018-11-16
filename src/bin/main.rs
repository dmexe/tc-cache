use std::path::PathBuf;

use tc_cache::{Config, Pull, Push, Error, Stats};
use clap::{App, Arg, SubCommand, AppSettings, ArgMatches};
use env_logger;
use log::info;

const PULL_COMMAND: &str = "pull";
const PUSH_COMMAND: &str = "push";
const PREFIX_ARG: &str = "prefix";
const HOME_ARG: &str = "home";
const DIRECTORY_ARG: &str = "directory";

fn run(app: ArgMatches) -> Result<(), Error> {
    env_logger::init();
    
    let cfg = app.value_of(HOME_ARG).map(Config::from).unwrap_or_else(Config::from_env)?;
    
    if let Some(pull) = app.subcommand_matches(PULL_COMMAND) {
        let directories = pull.values_of(DIRECTORY_ARG).unwrap();
        let directories = directories.map(PathBuf::from).collect::<Vec<_>>();
        let prefix = pull.value_of("prefix").map(PathBuf::from);
        let pull = Pull::new(&cfg, directories, prefix);
        
        pull.run()?;
    };
    
    if let Some(_push) = app.subcommand_matches(PUSH_COMMAND) {
        let push = Push::new(&cfg);
        
        push.run()?;
    }
    
    info!("{}", Stats::current());
    
    Ok(())
}

fn main() {
    let pull = SubCommand::with_name(PULL_COMMAND)
        .about("Pull a snapshot from a remote location")
        .arg(Arg::with_name(PREFIX_ARG)
            .short("p")
            .value_name("directory")
            .help("Extract snapshot into specific directory (default '/')")
        )
        .arg(Arg::with_name(DIRECTORY_ARG)
            .required(true)
            .min_values(1)
            .help("A list of directories to cache")
        );
    
    let push = SubCommand::with_name(PUSH_COMMAND)
        .about("Push cached directories into remote location");

    let app = App::new("TeamCity build cache CLI")
        .bin_name("tc-cache")
        .version("0.1")
        .setting(AppSettings::ColorAuto)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .setting(AppSettings::StrictUtf8)
        .arg(Arg::with_name(HOME_ARG)
            .short("-H")
            .value_name("directory")
            .help("Set working directory (default '~/.tc-cache')")
        )
        .subcommand(pull)
        .subcommand(push)
        .get_matches();
    
    run(app).unwrap();
}
