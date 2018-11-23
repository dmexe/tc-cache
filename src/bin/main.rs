use std::env;
use std::path::PathBuf;

use clap::{App, AppSettings, Arg, ArgMatches, SubCommand};
use env_logger;
use log::{error, info};
use tc_cache::{pretty, Config, Error, Pull, Push, Remote, Service, Stats, TeamCity};

const PULL_COMMAND: &str = "pull";
const PUSH_COMMAND: &str = "push";
const PREFIX_ARG: &str = "prefix";
const HOME_ARG: &str = "home";
const DIRECTORY_ARG: &str = "directory";
const TEAMCITY_PROPS_FILE_ARG: &str = "teamcity-build-properties-file";
const VERBOSE: &str = "verbose";

fn new_service(cfg: &Config, args: &ArgMatches) -> Result<Box<dyn Service>, Error> {
    let mut service: Option<Box<dyn Service>> = None;

    if let Some(path) = args.value_of(TEAMCITY_PROPS_FILE_ARG) {
        let teamcity = TeamCity::from_path(path)?;
        service = Some(teamcity.into_box());
    }

    let service = match service {
        Some(svc) => svc,
        None => {
            let env = env::vars().collect();
            TeamCity::from_env(&env)?.into_box()
        }
    };

    info!("{}", service);

    Ok(service)
}

fn new_config(args: &ArgMatches) -> Result<Config, Error> {
    let mut cfg = args
        .value_of(HOME_ARG)
        .map(Config::from)
        .unwrap_or_else(Config::from_env)?;

    if args.is_present(VERBOSE) {
        cfg.verbose(true);
    }
    
    Ok(cfg)
}

fn run(args: &ArgMatches) -> Result<(), Error> {
    env_logger::init();

    let cfg = new_config(&args)?;
    let service = new_service(&cfg, &args)?;
    let mut remote = Remote::new(&cfg);

    if let Some(pull) = args.subcommand_matches(PULL_COMMAND) {
        if service.is_uploadable() {
            remote.uri(service.remote_url())?;
            remote.prefix(service.project_id());
        }
        
        let directories = pull.values_of(DIRECTORY_ARG).unwrap();
        let directories = directories.map(PathBuf::from).collect::<Vec<_>>();
        let prefix = pull.value_of("prefix").map(PathBuf::from);
        let pull = Pull::new(&cfg, &remote, directories, prefix);

        return pull.run();
    };

    if let Some(_push) = args.subcommand_matches(PUSH_COMMAND) {
        if service.is_uploadable() {
            remote.uri(service.remote_url())?;
            remote.prefix(service.project_id());
        }

        let push = Push::new(&cfg, &remote);

        let (_, len) = push.run()?;
        if let Some(len) = len {
            info!("Snapshot size - {}", pretty::bytes(len));
        }

        return Ok(());
    }

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
                .help("Set working directory (default '~/.tc-cache')")
                .global(true),
        )
        .arg(
            Arg::with_name(TEAMCITY_PROPS_FILE_ARG)
                .hidden(true)
                .long("build-props")
                .value_name("file")
                .env("TEAMCITY_BUILD_PROPERTIES_FILE")
                .help("[advanced] override teamcity's build properties file")
                .global(true),
        )
        .arg(
            Arg::with_name(VERBOSE)
                .long("verbose")
                .short("v")
                .help("Enable debug output")
                .global(true),
        )
        .subcommand(pull)
        .subcommand(push)
        .get_matches();

    if let Err(err) = run(&app) {
        error!("{}", err);
    } else {
        info!("{}", Stats::current());
    }
}
