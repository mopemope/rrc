use anyhow::{Context, Result};
use dirs::home_dir;
use lazy_static::lazy_static;
use serde_derive::Deserialize;
use std::collections::{BTreeSet, HashMap};
use std::default::Default;
use std::fs::File;
use std::io::Read;
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
    pub each_cmd: Option<&'a Vec<&'a str>>,
    pub dry_run: bool,
}

#[derive(Debug, Deserialize, Clone)]
pub struct RepositoryConfig {
    #[serde(default = "default_root")]
    pub root: String,
    #[serde(default = "default_vec_str")]
    pub hosts: Vec<String>,
}

fn default_vec_str() -> Vec<String> {
    Vec::new()
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
        let each_cmd = None;
        let dry_run = false;
        Self {
            repos,
            query,
            look,
            profile,
            each_cmd,
            dry_run,
        }
    }
}

impl Default for RepositoryConfig {
    fn default() -> Self {
        let root = default_root();
        let hosts = vec![];
        Self { root, hosts }
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

    pub fn profile(&self, name: &str) -> Result<&RepositoryConfig> {
        if let Some(config) = self.repos.get(name) {
            Ok(config)
        } else {
            Err(anyhow::format_err!("profile '{}' not found", name))
        }
    }
}

pub fn parse_config(path: &str) -> Result<Config> {
    let mut config: Config = Default::default();
    if !path::Path::new(path).exists() {
        return Ok(config);
    }
    let mut config_toml = String::new();
    let mut file = File::open(path)?;
    file.read_to_string(&mut config_toml)?;

    let repos =
        from_str(&config_toml).with_context(|| format!("failed parse toml. path: {}", path))?;

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
