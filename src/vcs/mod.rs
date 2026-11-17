mod git;
mod hg;

use crate::utils::{run, run_with_work_dir};
use anyhow::{Result, format_err};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VCSBackend {
    GitBackend,
    MercurialBackend,
}

const MARKERS: &[(&str, VCSBackend)] = &[
    (git::MARKER, VCSBackend::GitBackend),
    (hg::MARKER, VCSBackend::MercurialBackend),
];

impl VCSBackend {
    /// Clones the remote repository `url` into `path`.
    pub fn clone_repository(&self, url: &str, path: &str) -> Result<()> {
        let clone = match self {
            VCSBackend::GitBackend => git::CLONE_CMD,
            VCSBackend::MercurialBackend => hg::CLONE_CMD,
        };
        let cmd: Vec<&str> = clone.iter().copied().chain([url, path]).collect();
        run(&cmd)
    }

    /// Updates the local repository at `path`.
    pub fn update(&self, path: &str) -> Result<()> {
        run_with_work_dir(self.update_command(), path)
    }

    /// The command that updates a local repository of this backend.
    pub fn update_command(&self) -> &'static [&'static str] {
        match self {
            VCSBackend::GitBackend => git::UPDATE_CMD,
            VCSBackend::MercurialBackend => hg::UPDATE_CMD,
        }
    }
}

/// Detects the backend of the remote repository `url` hosted on `host`.
/// This may probe the remote over the network.
pub fn detect_vcs(url: &str, host: &str) -> Result<VCSBackend> {
    git::from_str(url, host).or_else(|git_err| {
        hg::from_str(url).map_err(|hg_err| {
            format_err!("failed to detect vcs backend of {url} (git: {git_err:#}, hg: {hg_err:#})")
        })
    })
}

/// Detects the backend from the name of a directory entry (`.git`, `.hg`).
pub fn detect_vcs_from_path(name: &str) -> Option<VCSBackend> {
    MARKERS
        .iter()
        .find(|(marker, _)| *marker == name)
        .map(|(_, backend)| *backend)
}

/// Detects the backend of the local repository at `dir`, if it is one.
pub fn detect_vcs_from_dir(dir: &Path) -> Option<VCSBackend> {
    MARKERS
        .iter()
        .find(|(marker, _)| dir.join(marker).exists())
        .map(|(_, backend)| *backend)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_vcs_known_hosts() {
        for (url, host) in [
            ("https://github.com/user/repo", "github.com"),
            ("https://gitlab.com/user/repo", "gitlab.com"),
            ("git@github.com:user/repo.git", "github.com"),
            ("https://example.invalid/user/repo.git", "example.invalid"),
        ] {
            assert_eq!(detect_vcs(url, host).unwrap(), VCSBackend::GitBackend);
        }
    }

    #[test]
    fn test_detect_vcs_from_path() {
        assert_eq!(detect_vcs_from_path(".git"), Some(VCSBackend::GitBackend));
        assert_eq!(
            detect_vcs_from_path(".hg"),
            Some(VCSBackend::MercurialBackend)
        );
        assert_eq!(detect_vcs_from_path("invalid"), None);
    }

    #[test]
    fn test_detect_vcs_from_dir() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(detect_vcs_from_dir(dir.path()), None);
        assert_eq!(detect_vcs_from_dir(&dir.path().join("missing")), None);

        std::fs::create_dir(dir.path().join(".hg")).unwrap();
        assert_eq!(
            detect_vcs_from_dir(dir.path()),
            Some(VCSBackend::MercurialBackend)
        );

        // A `.git` file (worktree / submodule) also marks a git repository.
        std::fs::write(dir.path().join(".git"), "gitdir: elsewhere").unwrap();
        assert_eq!(
            detect_vcs_from_dir(dir.path()),
            Some(VCSBackend::GitBackend)
        );
    }
}
