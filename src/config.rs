use dirs::home_dir;
use failure::{err_msg, Error};
use lazy_static::lazy_static;
use serde_derive::Deserialize;
use std::collections::{BTreeSet, HashMap};
use std::default::Default;
use std::fs::File;
use std::io::{self, Read};
use std::{env, path};
use toml::from_str;

lazy_static! {
    pub static ref DEFAULT_REPO_ROOT: String = { default_root() };
}

#[derive(Debug, Clone)]
pub struct Config<'a> {
    pub repos: HashMap<String, RepositoryConfig>,
    pub query: String,
    pub look: bool,
    pub profile: Option<&'a str>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RepositoryConfig {
    #[serde(default = "default_root")]
    pub root: String,
}

fn default_root() -> String {
    match env::var("RRC_ROOT") {
        Ok(val) => val,
        Err(_) => {
            let home = home_dir().unwrap();
            home.join("repos").to_str().unwrap().to_owned()
        }
    }
}

impl Default for Config<'_> {
    fn default() -> Self {
        let mut repos = HashMap::new();
        let repo_config: RepositoryConfig = Default::default();
        repos.insert("default".to_owned(), repo_config);
        let query = String::new();
        let profile = None;
        let look = false;
        Self {
            repos,
            query,
            look,
            profile,
        }
    }
}

impl Default for RepositoryConfig {
    fn default() -> Self {
        let root = default_root();
        Self { root }
    }
}

impl Config<'_> {
    pub fn roots(&self) -> BTreeSet<&String> {
        let mut set = BTreeSet::new();
        for repo in self.repos.values() {
            set.insert(&repo.root);
        }
        set
    }

    pub fn profile(&self, name: &str) -> Result<&RepositoryConfig, Error> {
        if let Some(config) = self.repos.get(name) {
            Ok(config)
        } else {
            Err(err_msg(format!("profile '{}' not found", name)))
        }
    }
}

pub fn parse_config(path: &str) -> io::Result<Config> {
    let mut config: Config = Default::default();
    if !path::Path::new(path).exists() {
        return Ok(config);
    }
    let mut config_toml = String::new();
    let mut file = File::open(path)?;
    file.read_to_string(&mut config_toml)?;

    let repos = from_str(&config_toml).expect("toml parse error");
    config.repos = repos;
    Ok(config)
}

pub fn get_config_path() -> String {
    match env::var("RRC_CONFIG") {
        Ok(val) => val,
        Err(_) => {
            let home = home_dir().unwrap();
            home.join("rrc.toml").to_str().unwrap().to_owned()
        }
    }
}
