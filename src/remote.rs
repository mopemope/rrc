use crate::config::Config;
use crate::utils::chdir;
use crate::vcs::{detect_vcs, VCSOption};
use anyhow::{Context, Error, Result};
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

    fn from_str(s: &str) -> Result<SSHPath> {
        let cap = RE_SCP
            .captures(s)
            .with_context(|| format!("{} does not match", s))?;

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

fn parse_url(root: &str, raw_url: &str) -> Result<VCSOption> {
    let opt = if let Ok(url) = Url::parse(raw_url) {
        let url_path = Path::new(url.path());
        let parent = url_path
            .parent()
            .with_context(|| format!("unrecognized import path {}", raw_url))?;

        let parent = parent
            .file_name()
            .with_context(|| format!("unrecognized import path {}", raw_url))?;

        let host = url
            .host_str()
            .with_context(|| format!("unrecognized import path {}", raw_url))?;

        let root = Path::new(root);
        let file_stem = url_path
            .file_stem()
            .with_context(|| format!("unrecognized import path {}", raw_url))?;

        let dir = root.join(host).join(parent).join(file_stem);
        VCSOption {
            url: Some(raw_url.to_owned()),
            path: dir.to_str().unwrap().to_owned(),
            host: Some(host.to_owned()),
        }
    } else {
        if let Ok(ssh_path) = raw_url.parse() as Result<SSHPath> {
            let root = Path::new(root);
            let dir = root.join(ssh_path.host()).join(ssh_path.path());
            VCSOption {
                url: Some(raw_url.to_owned()),
                path: dir.to_str().unwrap().to_owned(),
                host: Some(ssh_path.host),
            }
        } else {
            let raw_url = format!("https://{}", raw_url);
            let url = Url::parse(&raw_url)?;
            let url_path = Path::new(url.path());

            let parent = url_path
                .parent()
                .with_context(|| format!("unrecognized import path {}", raw_url))?;

            let parent = parent
                .file_name()
                .with_context(|| format!("unrecognized import path {}", raw_url))?;

            let host = url
                .host_str()
                .with_context(|| format!("unrecognized import path {}", raw_url))?;

            let root = Path::new(root);
            let file_stem = url_path
                .file_stem()
                .with_context(|| format!("unrecognized import path {}", raw_url))?;

            let dir = root.join(host).join(parent).join(file_stem);

            VCSOption {
                url: Some(raw_url.to_owned()),
                path: dir.to_str().unwrap().to_owned(),
                host: Some(host.to_owned()),
            }
        }
    };
    debug!("{:?}", opt);
    Ok(opt)
}

pub fn get(config: &Config<'_>, raw_url: &str, update: bool) -> Result<()> {
    let profile = config.profile.unwrap_or("default");
    let repo_config = config.profile(profile)?;
    let root = &repo_config.root;
    debug!("repos_root {}", root);

    let opt = parse_url(root, raw_url)?;
    let vcs = detect_vcs(opt.url.as_ref().context("url not found")?)?;

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

    Ok(())
}

pub fn update_or_get(config: &Config<'_>, raw_url: &str) -> Result<()> {
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

fn sync_repo(_config: &Config<'_>, root: &str, raw_url: &str) -> Result<bool> {
    let opt = parse_url(root, raw_url)?;
    let vcs = detect_vcs(raw_url)?;
    if Path::new(&opt.path).exists() {
        vcs.update(&opt)?;
        return Ok(true);
    }
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
}
