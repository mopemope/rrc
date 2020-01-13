mod config;
mod local;
mod remote;
mod utils;
mod vcs;

use anyhow::{Context, Result};
use clap::{App, AppSettings, Arg, SubCommand};
use lazy_static::lazy_static;
use log::{debug, error};
use std::env;

lazy_static! {
    pub static ref CONFIG_PATH: String = { config::get_config_path() };
}

fn make_app() -> App<'static, 'static> {
    App::new("rrc")
        .version(env!("CARGO_PKG_VERSION"))
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .about("A manage remote repository clones")
        .arg(
            Arg::with_name("config")
                .multiple(false)
                .default_value(&CONFIG_PATH)
                .value_name("FILE")
                .short("c")
                .long("config")
                .help("Set config file"),
        )
        .subcommand(
            SubCommand::with_name("get")
                .about("Clone remote repository")
                .arg(
                    Arg::with_name("profile")
                        .multiple(false)
                        .value_name("profile")
                        .short("p")
                        .long("profile")
                        .help("Select profile"),
                )
                .arg(
                    Arg::with_name("update")
                        .multiple(false)
                        .short("u")
                        .long("update")
                        .help("Update local repository if cloned already"),
                )
                .arg(
                    Arg::with_name("look")
                        .multiple(false)
                        .short("l")
                        .long("look")
                        .help("Look after get"),
                )
                .arg(
                    Arg::with_name("url")
                        .required(true)
                        .multiple(true)
                        .value_name("repository url")
                        .help("Source repository url"),
                ),
        )
        .subcommand(
            SubCommand::with_name("list")
                .about("List local repositories")
                .arg(
                    Arg::with_name("profile")
                        .multiple(false)
                        .value_name("profile")
                        .short("p")
                        .long("profile")
                        .help("Select profile"),
                )
                .arg(
                    Arg::with_name("exact")
                        .multiple(false)
                        .value_name("query")
                        .short("e")
                        .long("exact")
                        .help("Perform an exact match"),
                ),
        )
        .subcommand(
            SubCommand::with_name("update")
                .about("Update local repositories")
                .arg(
                    Arg::with_name("profile")
                        .multiple(false)
                        .value_name("profile")
                        .short("p")
                        .long("profile")
                        .help("Select profile"),
                )
                .arg(
                    Arg::with_name("exact")
                        .multiple(false)
                        .value_name("query")
                        .short("e")
                        .long("exact")
                        .help("Perform an exact match"),
                ),
        )
        .subcommand(
            SubCommand::with_name("look")
                .about("Look local repository")
                .arg(
                    Arg::with_name("profile")
                        .multiple(false)
                        .value_name("profile")
                        .short("p")
                        .long("profile")
                        .help("Select profile"),
                )
                .arg(
                    Arg::with_name("exact")
                        .multiple(false)
                        .required(true)
                        .value_name("query")
                        .help("Perform an exact match"),
                ),
        )
        .subcommand(
            SubCommand::with_name("remove")
                .about("Remove local repositories")
                .arg(
                    Arg::with_name("profile")
                        .multiple(false)
                        .value_name("profile")
                        .short("p")
                        .long("profile")
                        .help("Select profile"),
                )
                .arg(
                    Arg::with_name("exact")
                        .multiple(false)
                        .required(true)
                        .value_name("query")
                        .help("Perform an exact match"),
                ),
        )
        .subcommand(
            SubCommand::with_name("each")
                .about("Execute command for each local repositories")
                .arg(
                    Arg::with_name("profile")
                        .multiple(false)
                        .value_name("profile")
                        .short("p")
                        .long("profile")
                        .help("Select profile"),
                )
                .arg(
                    Arg::with_name("dry-run")
                        .multiple(false)
                        .short("d")
                        .long("dry-run")
                        .help("Dry run"),
                )
                .arg(
                    Arg::with_name("exact")
                        .multiple(false)
                        .value_name("query")
                        .short("e")
                        .long("exact")
                        .help("Perform an exact match"),
                )
                .arg(
                    Arg::with_name("command")
                        .multiple(false)
                        .value_name("command")
                        .raw(true)
                        .required(true)
                        .help("Run command"),
                ),
        )
}

fn run() -> Result<()> {
    let app = make_app();
    let matches = app.get_matches();
    let config_path = matches.value_of("config").unwrap();

    let mut config = config::parse_config(config_path)?;
    debug!("config_path {} config {:?}", config_path, config);

    match matches.subcommand() {
        ("get", Some(m)) => {
            let urls = m.values_of("url").context("require repository url")?;
            let update = m.is_present("update");
            config.look = m.is_present("look");
            config.profile = m.value_of("profile");
            for url in urls {
                debug!("repository url {}", url);
                if update {
                    remote::update_or_get(&config, url)?;
                } else {
                    remote::get(&config, url, false)?;
                }
            }
            Ok(())
        }
        ("list", Some(m)) => {
            config.profile = m.value_of("profile");
            if let Some(query) = m.value_of("exact") {
                config.query = query.to_owned();
            }
            local::list(&config)
        }
        ("update", Some(m)) => {
            config.profile = m.value_of("profile");
            if let Some(query) = m.value_of("exact") {
                config.query = query.to_owned();
            }
            local::update(&config)
        }
        ("look", Some(m)) => {
            config.profile = m.value_of("profile");
            if let Some(query) = m.value_of("exact") {
                config.query = query.to_owned();
            }
            local::look(&config)
        }
        ("remove", Some(m)) => {
            config.profile = m.value_of("profile");
            if let Some(query) = m.value_of("exact") {
                config.query = query.to_owned();
            }
            local::remove(&config)
        }
        ("each", Some(m)) => {
            config.profile = m.value_of("profile");
            if let Some(query) = m.value_of("exact") {
                config.query = query.to_owned();
            }
            config.dry_run = m.is_present("dry-run");
            let cmd: Vec<&str> = m.values_of("command").unwrap().collect();
            config.each_cmd = Some(&cmd);
            local::each_exec(&config)
        }
        _ => unreachable!(),
    }
}

fn main() {
    env_logger::init();
    if let Err(err) = run() {
        error!("{}", err);
    }
}
