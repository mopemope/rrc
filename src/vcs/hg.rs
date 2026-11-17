use super::VCSBackend;
use crate::utils::run_silently;
use anyhow::{Result, bail};

pub const MARKER: &str = ".hg";
pub const CLONE_CMD: &[&str] = &["hg", "clone"];
pub const UPDATE_CMD: &[&str] = &["hg", "pull", "--update"];

pub fn from_str(url: &str) -> Result<VCSBackend> {
    if run_silently(&["hg", "identify", url])? {
        Ok(VCSBackend::MercurialBackend)
    } else {
        bail!("not a hg repository")
    }
}
