use super::VCSBackend;
use crate::utils::run_silently;
use anyhow::{Result, bail};

/// Hosts that only serve git repositories, so no network probe is needed.
const KNOWN_HOSTS: &[&str] = &["github.com", "gitlab.com", "bitbucket.org", "codeberg.org"];

pub const MARKER: &str = ".git";
pub const CLONE_CMD: &[&str] = &["git", "clone"];
pub const UPDATE_CMD: &[&str] = &["git", "pull", "--ff-only"];

pub fn from_str(url: &str, host: &str) -> Result<VCSBackend> {
    if KNOWN_HOSTS.contains(&host) || url.ends_with(".git") {
        return Ok(VCSBackend::GitBackend);
    }

    if run_silently(&["git", "ls-remote", url])? {
        Ok(VCSBackend::GitBackend)
    } else {
        bail!("not a git repository")
    }
}
