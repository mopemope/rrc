mod git;
mod hg;

use failure::{err_msg, Error};
use serde_derive::Deserialize;
use std::fmt::Debug;

#[derive(Debug, Deserialize, Clone)]
pub struct VCSOption {
    pub url: Option<String>,
    pub path: String,
    pub host: Option<String>,
    // pub recursive: bool,
    // pub shallow: bool,
    // pub silent: bool,
    // pub branch: String,
}

#[derive(Debug, Clone)]
pub enum VCSBackend {
    GitBackend,
    MercurialBackend,
}

impl VCSBackend {
    pub fn get_repository(&self, opt: &VCSOption) -> Result<(), Error> {
        match self {
            VCSBackend::GitBackend => git::get_repository(opt),
            VCSBackend::MercurialBackend => hg::get_repository(opt),
        }
    }
    pub fn update(&self, opt: &VCSOption) -> Result<(), Error> {
        match self {
            VCSBackend::GitBackend => git::update(opt),
            VCSBackend::MercurialBackend => hg::update(opt),
        }
    }
}

pub fn detect_vcs(url: &str) -> Result<VCSBackend, Error> {
    if let Ok(backend) = git::from_str(url) {
        Ok(backend)
    } else if let Ok(backend) = hg::from_str(url) {
        Ok(backend)
    } else {
        Err(err_msg(format!("fail detect vcs backend {}", url)))
    }
}

pub fn detect_vcs_from_path(path: &str) -> Option<VCSBackend> {
    if let Some(backend) = git::from_path(path) {
        Some(backend)
    } else if let Some(backend) = hg::from_path(path) {
        Some(backend)
    } else {
        None
    }
}
