use std::env;
use std::path::PathBuf;

use clap::{App, AppSettings, Arg, ArgMatches, SubCommand};
use env_logger;
use log::{error, info, LevelFilter};
use tc_cache::{Config, Error, Pull, Push, Service, ServiceFactory, Stats, Storage};

const PULL_COMMAND: &str = "pull";
const PUSH_COMMAND: &str = "push";
const PREFIX: &str = "prefix";
const HOME: &str = "home";
const DIRECTORY: &str = "directory";
const TEAMCITY_PROPS_FILE: &str = "teamcity-props-file";
const KEY: &str = "key";
const VERBOSE: &str = "verbose";

fn new_service(args: &ArgMatches) -> Result<Box<dyn Service>, Error> {
    let env = env::vars().collect();
    let service = ServiceFactory::from_env(&env, args.value_of(TEAMCITY_PROPS_FILE))?;
    info!("{}", service);
    Ok(service)
}

fn new_config(args: &ArgMatches) -> Result<Config, Error> {
    let mut cfg = args
        .value_of(HOME)
        .map(Config::from)
        .unwrap_or_else(Config::from_env)?;

    if args.is_present(VERBOSE) {
        cfg.verbose(true);
    }

    Ok(cfg)
}

fn new_storage(
    cfg: &Config,
    service: &Box<dyn Service>,
    args: &ArgMatches,
) -> Result<Storage, Error> {
    let mut storage = Storage::new(&cfg)
        .uri(service.remote_url())?
        .key_prefix(service.project_id())
        .uploadable(service.is_uploadable());

    if let Some(key_prefix) = args.value_of(KEY) {
        storage = storage.key_prefix(key_prefix);
    }

    storage.save()?;
    Ok(storage)
}

fn init_logger(args: &ArgMatches) {
    let log_level = if args.is_present(VERBOSE) {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    };

    env_logger::Builder::from_default_env()
        .filter_level(log_level)
        .init();
}

fn run(args: &ArgMatches) -> Result<(), Error> {
    init_logger(args);

    let cfg = new_config(&args)?;

    if let Some(pull) = args.subcommand_matches(PULL_COMMAND) {
        let service = new_service(&args)?;
        let storage = new_storage(&cfg, &service, &pull)?;

        let directories = pull.values_of(DIRECTORY).unwrap();
        let directories = directories.map(PathBuf::from).collect::<Vec<_>>();
        let prefix = pull.value_of("prefix").map(PathBuf::from);
        let pull = Pull::new(&cfg, &storage, &directories, prefix);

        return pull.run();
    };

    if let Some(_push) = args.subcommand_matches(PUSH_COMMAND) {
        let storage = Storage::load(&cfg.storage_file)?;
        let push = Push::new(&cfg, &storage);

        return push.run().map(|_| ());
    }

    Ok(())
}

fn main() {
    let pull = SubCommand::with_name(PULL_COMMAND)
        .about("Pull a snapshot from a remote location")
        .arg(
            Arg::with_name(TEAMCITY_PROPS_FILE)
                .hidden(true)
                .long("build-props")
                .value_name("file")
                .env("TEAMCITY_BUILD_PROPERTIES_FILE")
                .help("[advanced] override teamcity's build properties file"),
        )
        .arg(
            Arg::with_name(PREFIX)
                .long("prefix")
                .short("p")
                .value_name("directory")
                .help("Extract snapshot into specific directory (default '/')"),
        )
        .arg(
            Arg::with_name(KEY)
                .long("key")
                .short("k")
                .value_name("text")
                .help("Cache key prefix"),
        )
        .arg(
            Arg::with_name(DIRECTORY)
                .required(true)
                .min_values(1)
                .help("A list of directories to cache"),
        );

    let push =
        SubCommand::with_name(PUSH_COMMAND).about("Push cached directories into remote location");

    let app = App::new(env!("CARGO_PKG_DESCRIPTION"))
        .bin_name(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .setting(AppSettings::ColorAuto)
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .setting(AppSettings::StrictUtf8)
        .arg(
            Arg::with_name(HOME)
                .long("home")
                .short("d")
                .value_name("directory")
                .help("Set working directory (default '~/.tc-cache')")
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
