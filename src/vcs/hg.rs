use super::{VCSBackend, VCSOption};
use crate::utils::{run, run_silently, run_with_work_dir};
use failure::{err_msg, Error};

pub fn from_str(s: &str) -> Result<VCSBackend, Error> {
    match run_silently(&["hg", "identify", s]) {
        Ok(true) => Ok(VCSBackend::MercurialBackend),
        Ok(false) => Err(err_msg("not hg repository")),
        Err(e) => Err(e),
    }
}

pub fn get_repository(option: &VCSOption) -> Result<(), Error> {
    let url = option.url.clone().unwrap();
    match run(&["hg", "clone", &url, &option.path]) {
        Ok(_) => Ok(()),
        Err(e) => Err(e),
    }
}

pub fn update(option: &VCSOption) -> Result<(), Error> {
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
