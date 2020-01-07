use crate::config::Config;
use crate::utils::chdir;
use crate::vcs::{detect_vcs, VCSOption};
use failure::{format_err, Error, Fallible};
use lazy_static::lazy_static;
use log::debug;
use regex::Regex;
use std::fmt::{self, Debug};
use std::fs::create_dir_all;
use std::path::Path;
use std::str::FromStr;
use url::Url;

lazy_static! {
    static ref RE_SCP: Regex =
        Regex::new(r"^((?:[^@]+@)?)([^:]+):/?(.+)$").expect("should be a valid regex pattern");
}

#[derive(Debug)]
struct SSHPath {
    user: String,
    host: String,
    path: String,
}

impl SSHPath {
    pub fn user(&self) -> &str {
        &self.user
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

impl FromStr for SSHPath {
    type Err = Error;

    fn from_str(s: &str) -> Fallible<SSHPath> {
        let cap = RE_SCP
            .captures(s)
            .ok_or_else(|| format_err!("does not match"))?;

        let user = cap
            .get(1)
            .and_then(|s| {
                if s.as_str() != "" {
                    Some(s.as_str())
                } else {
                    None
                }
            })
            .map(|s| s.trim_end_matches('@'))
            .unwrap_or("git")
            .to_owned();
        let host = cap.get(2).unwrap().as_str().to_owned();
        let path = cap
            .get(3)
            .unwrap()
            .as_str()
            .trim_end_matches(".git")
            .to_owned();
        Ok(SSHPath { user, host, path })
    }
}

impl fmt::Display for SSHPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}@{}:{}.git", self.user, self.host, self.path)
    }
}

pub fn get(config: &Config<'_>, raw_url: &str, update: bool) -> Result<(), Error> {
    let profile = config.profile.unwrap_or("default");
    let repo_config = config.profile(profile)?;
    let root = &repo_config.root;
    debug!("repos_root {}", root);

    if let Ok(vcs) = detect_vcs(raw_url) {
        let opt = if let Ok(url) = Url::parse(raw_url) {
            let url_path = Path::new(url.path());
            let parent = url_path.parent().unwrap().file_name().unwrap();
            let host = url.host_str().unwrap();
            let root = Path::new(root);
            let file_stem = url_path.file_stem().unwrap();
            let dir = root.join(host).join(parent).join(file_stem);

            VCSOption {
                url: Some(raw_url.to_owned()),
                path: dir.to_str().unwrap().to_owned(),
                host: Some(host.to_owned()),
            }
        } else {
            let ssh_path: SSHPath = raw_url.parse()?;
            let root = Path::new(root);
            let dir = root.join(ssh_path.host()).join(ssh_path.path());
            VCSOption {
                url: Some(raw_url.to_owned()),
                path: dir.to_str().unwrap().to_owned(),
                host: Some(ssh_path.host),
            }
        };

        if update && Path::new(&opt.path).exists() {
            vcs.update(&opt)?;
            if config.look {
                chdir(&opt.path)?;
            }
        } else {
            if !Path::new(&opt.path).exists() {
                create_dir_all(&opt.path)?;
            }
            vcs.get_repository(&opt)?;
            if config.look {
                chdir(&opt.path)?;
            }
        }
    }
    Ok(())
}

pub fn update_or_get(config: &Config<'_>, raw_url: &str) -> Result<(), Error> {
    if let Some(profile) = config.profile {
        let repo_config = config.profile(profile)?;
        if sync_repo(config, &repo_config.root, raw_url)? {
            return Ok(());
        }
    } else {
        for root in config.roots() {
            if sync_repo(config, &root, raw_url)? {
                return Ok(());
            }
        }
    }
    // not found
    get(config, raw_url, true)
}

fn sync_repo(config: &Config<'_>, root: &str, raw_url: &str) -> Result<bool, Error> {
    if let Ok(vcs) = detect_vcs(raw_url) {
        let opt = if let Ok(url) = Url::parse(raw_url) {
            let url_path = Path::new(url.path());
            let parent = url_path.parent().unwrap().file_name().unwrap();
            let host = url.host_str().unwrap();
            let root = Path::new(root);
            let file_stem = url_path.file_stem().unwrap();
            let dir = root.join(host).join(parent).join(file_stem);

            VCSOption {
                url: Some(raw_url.to_owned()),
                path: dir.to_str().unwrap().to_owned(),
                host: Some(host.to_owned()),
            }
        } else {
            let ssh_path: SSHPath = raw_url.parse()?;
            let root = Path::new(root);
            let dir = root.join(ssh_path.host()).join(ssh_path.path());
            VCSOption {
                url: Some(raw_url.to_owned()),
                path: dir.to_str().unwrap().to_owned(),
                host: Some(ssh_path.host),
            }
        };
        if Path::new(&opt.path).exists() {
            vcs.update(&opt)?;
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
}
