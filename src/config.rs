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
    pub static ref DEFAULT_REPO_ROOT: String = default_root();
}

#[derive(Debug, Clone)]
pub struct Config<'a> {
    pub repos: HashMap<String, RepositoryConfig>,
    pub query: String,
    pub look: bool,
    pub profile: Option<&'a str>,
    pub each_cmd: Option<&'a Vec<&'a str>>,
    pub dry_run: bool,
    pub hosts: HashMap<String, String>,
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
        let hosts = HashMap::new();
        Self {
            repos,
            query,
            look,
            profile,
            each_cmd,
            dry_run,
            hosts,
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

pub fn parse_config(path: &str) -> Result<Config<'_>> {
    let mut config: Config = Default::default();
    if !path::Path::new(path).exists() {
        return Ok(config);
    }
    let mut config_toml = String::new();
    let mut file = File::open(path)?;
    file.read_to_string(&mut config_toml)?;

    let repos: HashMap<String, RepositoryConfig> =
        from_str(&config_toml).with_context(|| format!("failed parse toml. path: {}", path))?;

    for (_, repo_conf) in repos.iter() {
        let root = &repo_conf.root;
        for host in &repo_conf.hosts {
            if !config.hosts.contains_key(host) {
                config.hosts.insert(host.to_string(), root.to_owned());
            }
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_profile() {
        let mut config = Config::default();
        let repo_config = RepositoryConfig {
            root: "/foo/bar".to_string(),
            hosts: vec!["github.com".to_string()],
        };
        config.repos.insert("test".to_string(), repo_config);

        let profile = config.profile("test").unwrap();
        assert_eq!(profile.root, "/foo/bar");

        assert!(config.profile("invalid").is_err());
    }

    #[test]
    fn test_config_roots() {
        let mut config = Config::default();
        config.repos.clear();
        config.repos.insert(
            "a".to_string(),
            RepositoryConfig {
                root: "/root1".to_string(),
                hosts: vec![],
            },
        );
        config.repos.insert(
            "b".to_string(),
            RepositoryConfig {
                root: "/root2".to_string(),
                hosts: vec![],
            },
        );
        config.repos.insert(
            "c".to_string(),
            RepositoryConfig {
                root: "/root1".to_string(),
                hosts: vec![],
            },
        );

        let roots = config.roots();
        assert_eq!(roots.len(), 2);
        assert!(roots.contains(&"/root1".to_string()));
        assert!(roots.contains(&"/root2".to_string()));
    }
}
