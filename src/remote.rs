use crate::config::Config;
use crate::utils::{chdir, expand_home};
use crate::vcs::{VCSBackend, detect_vcs, detect_vcs_from_dir};
use anyhow::{Context, Result, bail};
use log::debug;
use std::fs::create_dir_all;
use std::path::{Component, Path};
use url::Url;

/// A remote repository resolved to its local clone location.
#[derive(Debug)]
struct Remote {
    /// The url passed to the VCS.
    url: String,
    host: String,
    /// The local clone path (`<root>/<host>/<path>`).
    path: String,
}

/// Parses scp-like syntax (`[user@]host:path`) the way git does: it is only
/// recognized when there is no slash before the first colon.
fn parse_scp_like(s: &str) -> Option<(&str, &str)> {
    let (head, path) = s.split_once(':')?;
    if head.contains('/') {
        return None;
    }
    let host = head.rsplit_once('@').map_or(head, |(_, host)| host);
    if host.is_empty() || path.is_empty() {
        return None;
    }
    Some((host, path))
}

/// Normalizes a repository path: trims slashes and a single trailing `.git`.
fn repo_path(raw_url: &str, path: &str) -> Result<String> {
    let path = path.trim_matches('/');
    let path = path
        .strip_suffix(".git")
        .unwrap_or(path)
        .trim_end_matches('/');
    let valid = !path.is_empty()
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)));
    if !valid {
        bail!("invalid repository path in {raw_url}");
    }
    Ok(path.to_owned())
}

/// Splits `raw_url` into the url passed to the VCS, its host and repository path.
fn split_url(raw_url: &str) -> Result<(String, String, String)> {
    let parse_url = |url: String| -> Result<(String, String, String)> {
        let parsed = Url::parse(&url).with_context(|| format!("invalid url {raw_url}"))?;
        let host = parsed
            .host_str()
            .with_context(|| format!("unrecognized import path {raw_url}"))?
            .to_owned();
        let path = repo_path(raw_url, parsed.path())?;
        Ok((url, host, path))
    };

    if raw_url.contains("://") {
        parse_url(raw_url.to_owned())
    } else if let Some((host, path)) = parse_scp_like(raw_url) {
        // Host names are case-insensitive; normalize like `Url` does.
        Ok((
            raw_url.to_owned(),
            host.to_ascii_lowercase(),
            repo_path(raw_url, path)?,
        ))
    } else if raw_url.trim_matches('/').split('/').count() == 2 {
        // `user/repo` is a shorthand for GitHub.
        parse_url(format!("https://github.com/{raw_url}"))
    } else {
        parse_url(format!("https://{raw_url}"))
    }
}

fn parse_url(config: &Config<'_>, default_root: &str, raw_url: &str) -> Result<Remote> {
    let (url, host, path) = split_url(raw_url)?;
    let root = config.hosts.get(&host).map_or(default_root, String::as_str);
    let root = expand_home(root).context("failed to expand home")?;
    let dir = root.join(&host).join(path);
    let remote = Remote {
        url,
        host,
        path: dir.to_str().context("non UTF-8 path")?.to_owned(),
    };
    debug!("{remote:?}");
    Ok(remote)
}

fn update_local(config: &Config<'_>, remote: &Remote, backend: VCSBackend) -> Result<()> {
    println!("update {}", remote.path);
    backend.update(&remote.path)?;
    if config.look {
        chdir(&remote.path)?;
    }
    Ok(())
}

/// Clones `raw_url` into the root of the selected profile.
/// An already cloned repository is left as it is.
pub fn get(config: &Config<'_>, raw_url: &str) -> Result<()> {
    let profile = config.profile.unwrap_or("default");
    let root = &config.profile(profile)?.root;
    debug!("default repos_root {root}");

    let remote = parse_url(config, root, raw_url)?;
    let path = Path::new(&remote.path);
    if detect_vcs_from_dir(path).is_some() {
        println!("exists {}", remote.path);
    } else {
        let vcs = detect_vcs(&remote.url, &remote.host)?;
        if let Some(parent) = path.parent() {
            create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        println!("clone {} -> {}", remote.url, remote.path);
        vcs.clone_repository(&remote.url, &remote.path)?;
    }
    if config.look {
        chdir(path)?;
    }
    Ok(())
}

/// Updates the local clone of `raw_url` if one exists in any root (or in the
/// root of the selected profile), and clones it otherwise.
pub fn update_or_get(config: &Config<'_>, raw_url: &str) -> Result<()> {
    let roots: Vec<&String> = match config.profile {
        Some(profile) => vec![&config.profile(profile)?.root],
        None => config.roots().into_iter().collect(),
    };
    for root in roots {
        let remote = parse_url(config, root, raw_url)?;
        // Detect from the local clone: no network round trip is needed.
        if let Some(backend) = detect_vcs_from_dir(Path::new(&remote.path)) {
            return update_local(config, &remote, backend);
        }
    }
    // not found
    get(config, raw_url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn test_parse_scp_like() {
        assert_eq!(
            parse_scp_like("git@github.com:mopemope/rrc.git"),
            Some(("github.com", "mopemope/rrc.git"))
        );
        assert_eq!(
            parse_scp_like("github.com:mopemope/rrc"),
            Some(("github.com", "mopemope/rrc"))
        );
        assert_eq!(
            parse_scp_like("user@host:/path/to/repo"),
            Some(("host", "/path/to/repo"))
        );
        // A slash before the colon means a local path, as in git.
        assert_eq!(parse_scp_like("./foo:bar"), None);
        assert_eq!(parse_scp_like("mopemope/rrc"), None);
        assert_eq!(parse_scp_like(":path"), None);
        assert_eq!(parse_scp_like("host:"), None);
    }

    fn path_of(config: &Config<'_>, raw_url: &str) -> String {
        parse_url(config, "/tmp/repos", raw_url).unwrap().path
    }

    #[test]
    fn test_parse_url() {
        let config = Config::default();

        // HTTPS URL
        let remote = parse_url(&config, "/tmp/repos", "https://github.com/mopemope/rrc").unwrap();
        assert_eq!(remote.url, "https://github.com/mopemope/rrc");
        assert_eq!(remote.host, "github.com");
        assert_eq!(remote.path, "/tmp/repos/github.com/mopemope/rrc");

        // Git SCP-like URL
        let remote = parse_url(&config, "/tmp/repos", "git@github.com:mopemope/rrc.git").unwrap();
        assert_eq!(remote.url, "git@github.com:mopemope/rrc.git");
        assert_eq!(remote.path, "/tmp/repos/github.com/mopemope/rrc");

        // The host is normalized to lower case, as for URLs.
        let remote = parse_url(&config, "/tmp/repos", "git@GitHub.com:mopemope/rrc").unwrap();
        assert_eq!(remote.host, "github.com");
        assert_eq!(remote.path, "/tmp/repos/github.com/mopemope/rrc");

        // SCP-like URL without user
        let remote = parse_url(&config, "/tmp/repos", "github.com:mopemope/rrc").unwrap();
        assert_eq!(remote.url, "github.com:mopemope/rrc");
        assert_eq!(remote.path, "/tmp/repos/github.com/mopemope/rrc");

        // Short GitHub URL
        let remote = parse_url(&config, "/tmp/repos", "mopemope/rrc").unwrap();
        assert_eq!(remote.url, "https://github.com/mopemope/rrc");
        assert_eq!(remote.path, "/tmp/repos/github.com/mopemope/rrc");

        // Host and path without scheme
        let remote = parse_url(&config, "/tmp/repos", "gitlab.com/a/b").unwrap();
        assert_eq!(remote.url, "https://gitlab.com/a/b");
        assert_eq!(remote.path, "/tmp/repos/gitlab.com/a/b");

        // ssh:// URL
        assert_eq!(
            path_of(&config, "ssh://git@example.com:2222/a/b.git"),
            "/tmp/repos/example.com/a/b"
        );
    }

    #[test]
    fn test_parse_url_keeps_dots_in_name() {
        let config = Config::default();
        assert_eq!(
            path_of(&config, "https://github.com/socketio/socket.io"),
            "/tmp/repos/github.com/socketio/socket.io"
        );
        assert_eq!(
            path_of(&config, "git@github.com:socketio/socket.io.git"),
            "/tmp/repos/github.com/socketio/socket.io"
        );
        assert_eq!(
            path_of(&config, "https://github.com/a/b.git/"),
            "/tmp/repos/github.com/a/b"
        );
        // Only a single `.git` suffix is removed.
        assert_eq!(
            path_of(&config, "https://github.com/a/b.git.git"),
            "/tmp/repos/github.com/a/b.git"
        );
    }

    #[test]
    fn test_parse_url_rejects_invalid_paths() {
        let config = Config::default();
        for raw_url in [
            "https://github.com",
            "https://github.com/",
            "https://github.com/.git",
            "host:../../etc",
            "host:./repo",
        ] {
            assert!(
                parse_url(&config, "/tmp/repos", raw_url).is_err(),
                "{raw_url} should be rejected"
            );
        }
    }

    #[test]
    fn test_parse_url_host_root() {
        let mut config = Config::default();
        config
            .hosts
            .insert("github.com".to_string(), "/gh/src".to_string());
        assert_eq!(
            path_of(&config, "mopemope/rrc"),
            "/gh/src/github.com/mopemope/rrc"
        );
        assert_eq!(
            path_of(&config, "gitlab.com/a/b"),
            "/tmp/repos/gitlab.com/a/b"
        );
    }
}
