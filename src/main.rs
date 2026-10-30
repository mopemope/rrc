mod config;
mod local;
mod remote;
mod utils;
mod vcs;

use anyhow::Result;
use clap::{Parser, Subcommand};
use log::{debug, error};

#[derive(Debug, Parser)]
#[command(
    name = "rrc",
    version,
    about = "A manage remote repository clones",
    arg_required_else_help = true
)]
struct Cli {
    /// Set config file
    #[arg(short, long, value_name = "FILE", default_value_t = config::get_config_path())]
    config: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Clone remote repository
    Get {
        /// Select profile
        #[arg(short, long, value_name = "profile")]
        profile: Option<String>,

        /// Update local repository if cloned already
        #[arg(short, long)]
        update: bool,

        /// Look after get
        #[arg(short, long)]
        look: bool,

        /// Source repository url
        #[arg(required = true, value_name = "repository url")]
        url: Vec<String>,
    },
    /// List local repositories
    List {
        /// Select profile
        #[arg(short, long, value_name = "profile")]
        profile: Option<String>,

        /// Perform an exact match
        #[arg(short, long, value_name = "query")]
        exact: Option<String>,
    },
    /// Update local repositories
    Update {
        /// Select profile
        #[arg(short, long, value_name = "profile")]
        profile: Option<String>,

        /// Perform an exact match
        #[arg(short, long, value_name = "query")]
        exact: Option<String>,
    },
    /// Look local repository
    Look {
        /// Select profile
        #[arg(short, long, value_name = "profile")]
        profile: Option<String>,

        /// Perform an exact match
        #[arg(value_name = "query")]
        exact: String,
    },
    /// Remove local repositories
    Remove {
        /// Select profile
        #[arg(short, long, value_name = "profile")]
        profile: Option<String>,

        /// Perform an exact match
        #[arg(value_name = "query")]
        exact: String,
    },
    /// Execute command for each local repositories
    Each {
        /// Select profile
        #[arg(short, long, value_name = "profile")]
        profile: Option<String>,

        /// Dry run
        #[arg(short, long)]
        dry_run: bool,

        /// Perform an exact match
        #[arg(short, long, value_name = "query")]
        exact: Option<String>,

        /// Run command
        #[arg(raw = true, required = true, value_name = "command")]
        command: Vec<String>,
    },
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let config_path = &cli.config;

    let mut config = config::parse_config(config_path)?;
    debug!("config_path {config_path} config {config:?}");

    match &cli.command {
        Commands::Get {
            profile,
            update,
            look,
            url,
        } => {
            config.look = *look;
            config.profile = profile.as_deref();
            for url in url {
                debug!("repository url {url}");
                if *update {
                    remote::update_or_get(&config, url)?;
                } else {
                    remote::get(&config, url, false)?;
                }
            }
            Ok(())
        }
        Commands::List { profile, exact } => {
            config.profile = profile.as_deref();
            if let Some(query) = exact {
                config.query = query.to_owned();
            }
            local::list(&config)
        }
        Commands::Update { profile, exact } => {
            config.profile = profile.as_deref();
            if let Some(query) = exact {
                config.query = query.to_owned();
            }
            local::update(&config)
        }
        Commands::Look { profile, exact } => {
            config.profile = profile.as_deref();
            config.query = exact.to_owned();
            local::look(&config)
        }
        Commands::Remove { profile, exact } => {
            config.profile = profile.as_deref();
            config.query = exact.to_owned();
            local::remove(&config)
        }
        Commands::Each {
            profile,
            dry_run,
            exact,
            command,
        } => {
            config.profile = profile.as_deref();
            if let Some(query) = exact {
                config.query = query.to_owned();
            }
            config.dry_run = *dry_run;
            let cmd: Vec<&str> = command.iter().map(String::as_str).collect();
            config.each_cmd = Some(&cmd);
            local::each_exec(&config)
        }
    }
}

fn main() {
    env_logger::init();
    if let Err(err) = run() {
        error!("{err}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn verify_cli() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parse_each_command() {
        let cli =
            Cli::try_parse_from(["rrc", "each", "-d", "-e", "foo", "--", "git", "-v"]).unwrap();
        match cli.command {
            Commands::Each {
                dry_run,
                exact,
                command,
                ..
            } => {
                assert!(dry_run);
                assert_eq!(exact.as_deref(), Some("foo"));
                assert_eq!(command, vec!["git", "-v"]);
            }
            _ => panic!("unexpected subcommand"),
        }
    }
}
