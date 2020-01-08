use super::{VCSBackend, VCSOption};
use crate::utils::{run, run_silently, run_with_work_dir};
use anyhow::Result;
use log::debug;
use url::Url;

pub fn from_str(s: &str) -> Result<VCSBackend> {
    if let Ok(url) = Url::parse(s) {
        if let Some(host) = url.host_str() {
            if host == "github.com" {
                return Ok(VCSBackend::GitBackend);
            }
            if host == "gitlab.com" {
                return Ok(VCSBackend::GitBackend);
            }
        }
    }

    match run_silently(&["git", "ls-remote", s]) {
        Ok(true) => Ok(VCSBackend::GitBackend),
        Ok(false) => Err(anyhow::format_err!("not git repository")),
        Err(e) => Err(e),
    }
}

pub fn get_repository(option: &VCSOption) -> Result<()> {
    let url = option.url.clone().unwrap();
    match run(&["git", "clone", &url, &option.path]) {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

pub fn update(option: &VCSOption) -> Result<()> {
    match run_with_work_dir(&["git", "pull", "--ff-only"], &option.path) {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

pub fn from_path(path: &str) -> Option<VCSBackend> {
    if path == ".git" {
        Some(VCSBackend::GitBackend)
    } else {
        None
    }
}
