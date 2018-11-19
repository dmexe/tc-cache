use std::path::PathBuf;

use clap::{App, AppSettings, Arg, ArgMatches, SubCommand};
use env_logger;
use log::{error, info};
use tc_cache::{pretty, Config, Environment, Error, Pull, Push, Stats, TeamCityEnv};

const PULL_COMMAND: &str = "pull";
const PUSH_COMMAND: &str = "push";
const PREFIX_ARG: &str = "prefix";
const HOME_ARG: &str = "home";
const DIRECTORY_ARG: &str = "directory";
const TEAMCITY_PROPS_FILE_ARG: &str = "teamcity-build-properties-file";

fn run(app: &ArgMatches) -> Result<(), Error> {
    env_logger::init();

    let cfg = app
        .value_of(HOME_ARG)
        .map(Config::from)
        .unwrap_or_else(Config::from_env)?;

    let mut env: Option<Box<dyn Environment>> = None;

    if let Some(path) = app.value_of(TEAMCITY_PROPS_FILE_ARG) {
        env = TeamCityEnv::from_path(path).map(TeamCityEnv::into_box)
    }

    let environment = match env.or_else(|| TeamCityEnv::from_env().map(TeamCityEnv::into_box)) {
        Some(env) => env,
        None => return Err(Error::unrecognized_environment("Unknown")),
    };

    info!("{}", environment);

    if let Some(pull) = app.subcommand_matches(PULL_COMMAND) {
        let directories = pull.values_of(DIRECTORY_ARG).unwrap();
        let directories = directories.map(PathBuf::from).collect::<Vec<_>>();
        let prefix = pull.value_of("prefix").map(PathBuf::from);
        let pull = Pull::new(&cfg, directories, prefix);

        pull.run()?;
    };

    if let Some(_push) = app.subcommand_matches(PUSH_COMMAND) {
        let push = Push::new(&cfg);

        let (_, len) = push.run()?;
        if let Some(len) = len {
            info!("Snapshot size - {}", pretty::bytes(len));
        }
    }

    info!("{}", Stats::current());

    Ok(())
}

fn main() {
    let pull = SubCommand::with_name(PULL_COMMAND)
        .about("Pull a snapshot from a remote location")
        .arg(
            Arg::with_name(PREFIX_ARG)
                .long("prefix")
                .short("p")
                .value_name("directory")
                .help("Extract snapshot into specific directory (default '/')"),
        )
        .arg(
            Arg::with_name(DIRECTORY_ARG)
                .required(true)
                .min_values(1)
                .help("A list of directories to cache"),
        );

    let push =
        SubCommand::with_name(PUSH_COMMAND).about("Push cached directories into remote location");

    let app = App::new("TeamCity build cache CLI")
        .bin_name("tc-cache")
        .version("0.1")
        .setting(AppSettings::ColorAuto)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .setting(AppSettings::StrictUtf8)
        .arg(
            Arg::with_name(HOME_ARG)
                .long("home")
                .short("d")
                .value_name("directory")
                .help("Set working directory (default '~/.tc-cache')"),
        )
        .arg(
            Arg::with_name(TEAMCITY_PROPS_FILE_ARG)
                .hidden(true)
                .long("build-props")
                .value_name("file")
                .env("TEAMCITY_BUILD_PROPERTIES_FILE")
                .help("[advanced] override teamcity's build properties file"),
        )
        .subcommand(pull)
        .subcommand(push)
        .get_matches();

    if let Err(err) = run(&app) {
        error!("{}", err);
    }
}
