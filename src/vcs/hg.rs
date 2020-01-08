use super::{VCSBackend, VCSOption};
use crate::utils::{run, run_silently, run_with_work_dir};
use anyhow::Result;

pub fn from_str(s: &str) -> Result<VCSBackend> {
    match run_silently(&["hg", "identify", s]) {
        Ok(true) => Ok(VCSBackend::MercurialBackend),
        Ok(false) => Err(anyhow::format_err!("not hg repository")),
        Err(e) => Err(e),
    }
}

pub fn get_repository(option: &VCSOption) -> Result<()> {
    let url = option.url.clone().unwrap();
    match run(&["hg", "clone", &url, &option.path]) {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

pub fn update(option: &VCSOption) -> Result<()> {
    match run_with_work_dir(&["hg", "pull", "--update"], &option.path) {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

pub fn from_path(path: &str) -> Option<VCSBackend> {
    if path == ".hg" {
        Some(VCSBackend::MercurialBackend)
    } else {
        None
    }
}
