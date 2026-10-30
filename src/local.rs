use crate::config::Config;
use crate::utils::{chdir, confirm, expand_home, run_with_work_dir};
use crate::vcs::{VCSBackend, VCSOption, detect_vcs_from_path};
use anyhow::Result;
use log::{debug, error};
use rayon::prelude::*;
use std::fmt::{self, Debug, Formatter};
use std::fs;
use std::path::Path;

#[derive(Clone)]
pub struct LocalRepository {
    pub path: String,
    pub relpath: String,
    pub backend: VCSBackend,
}

impl Debug for LocalRepository {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        f.debug_struct("LocalRepository")
            .field("path", &self.path)
            .field("backend", &self.backend)
            .finish()
    }
}

fn walk_repository(root_path: &str, repos: &mut Vec<LocalRepository>) -> Result<()> {
    let Some(root) = expand_home(root_path) else {
        error!("failed to expand home. path:{root_path:?}");
        return Ok(());
    };
    if let Err(e) = fs::metadata(&root) {
        error!("{e} path:{root:?}");
        return Ok(());
    }

    repos.append(&mut scan_dir(&root, &root));
    Ok(())
}

/// Recursively scans `dir` for repositories in parallel.
/// Once a directory is identified as a repository, its descendants are not scanned.
fn scan_dir(root: &Path, dir: &Path) -> Vec<LocalRepository> {
    let mut entries: Vec<fs::DirEntry> = match fs::read_dir(dir) {
        Ok(entries) => entries.flatten().collect(),
        Err(e) => {
            debug!("{e} path:{dir:?}");
            return vec![];
        }
    };

    let found_backend = entries
        .iter()
        .find_map(|entry| entry.file_name().to_str().and_then(detect_vcs_from_path));
    if let Some(backend) = found_backend {
        // Use std::path::Path to calculate relative path safely
        return match (
            dir.to_str(),
            dir.strip_prefix(root).ok().and_then(Path::to_str),
        ) {
            (Some(path), Some(relpath)) => vec![LocalRepository {
                path: path.to_owned(),
                relpath: relpath.to_owned(),
                backend,
            }],
            _ => vec![],
        };
    }

    entries.sort_by_key(fs::DirEntry::file_name);
    entries
        .par_iter()
        // Symbolic links are not followed.
        .filter(|entry| entry.file_type().is_ok_and(|t| t.is_dir()))
        // Skip recursing into hidden directories.
        .filter(|entry| !entry.file_name().as_encoded_bytes().starts_with(b"."))
        .flat_map_iter(|entry| scan_dir(root, &entry.path()))
        .collect()
}

fn walk_repositories(config: &Config<'_>) -> Result<Vec<LocalRepository>> {
    let mut result: Vec<LocalRepository> = vec![];
    for root in config.roots() {
        walk_repository(root, &mut result)?;
    }
    Ok(result)
}

fn list_repos(config: &Config<'_>, profile: &str) -> Result<Vec<LocalRepository>> {
    let repo_config = config.profile(profile)?;
    let mut result: Vec<LocalRepository> = vec![];
    walk_repository(&repo_config.root, &mut result)?;
    Ok(result)
}

fn each_repo(
    config: &Config<'_>,
    f: fn(&Config<'_>, &Vec<&LocalRepository>) -> Result<()>,
) -> Result<()> {
    let repos = if let Some(profile) = config.profile {
        list_repos(config, profile)?
    } else {
        walk_repositories(config)?
    };
    let fuzzy = FuzzyVec::from_vec(repos);
    let repos = fuzzy.search(&config.query);
    f(config, &repos)
}

pub fn list(config: &Config<'_>) -> Result<()> {
    each_repo(config, |_, repos| {
        for repo in repos {
            println!("{}", repo.path);
        }
        Ok(())
    })
}

pub fn update(config: &Config<'_>) -> Result<()> {
    each_repo(config, |_, repos| {
        repos.par_iter().for_each(|repo| {
            let opt = VCSOption {
                url: None,
                path: repo.path.clone(),
            };
            println!("update {}", opt.path);
            if let Err(e) = repo.backend.update(&opt) {
                error!("Failed to update {}: {}", repo.path, e);
            }
        });
        Ok(())
    })
}

pub fn look(config: &Config<'_>) -> Result<()> {
    each_repo(config, |config, repos| {
        if repos.is_empty() {
            Err(anyhow::format_err!("{} not found", config.query))
        } else {
            let path = &repos[0].path;
            chdir(path)?;
            Ok(())
        }
    })
}

pub fn remove(config: &Config<'_>) -> Result<()> {
    each_repo(config, |_, repos| {
        for repo in repos {
            println!("{}", repo.path);
            if confirm("do you want to remove this? [Y/n]", "Y", Some("Y"))? {
                fs::remove_dir_all(&repo.path)?;
                println!("removed {}", repo.path);
            }
            println!();
        }
        Ok(())
    })
}

pub fn each_exec(config: &Config<'_>) -> Result<()> {
    each_repo(config, |config, repos| {
        if let Some(cmd) = config.each_cmd {
            if config.dry_run {
                for repo in repos {
                    println!("{} : dry-run {:?} ", repo.path, cmd);
                }
            } else {
                repos.par_iter().for_each(|repo| {
                    println!("{} : exec {:?} ", repo.path, cmd);
                    if let Err(e) = run_with_work_dir(cmd, &repo.path) {
                        error!("Failed to exec in {}: {}", repo.path, e);
                    }
                });
            }
        }
        Ok(())
    })
}

///
/// from github.com/nuta/nsh
/// A ordered `Vec` which supports fuzzy search.
///
struct FuzzyVec {
    /// The *unordered* array of a haystack.
    entries: Vec<LocalRepository>,
}

impl FuzzyVec {
    /// Creates a `FuzzyVec` from `entries`.
    pub fn from_vec(entries: Vec<LocalRepository>) -> FuzzyVec {
        FuzzyVec { entries }
    }

    /// Searches entiries for `query` in a fuzzy way and returns the result
    /// ordered by the similarity.
    pub fn search(&self, query: &str) -> Vec<&LocalRepository> {
        fuzzy_search(&self.entries, query)
    }
}

fn fuzzy_search<'a>(entries: &'a [LocalRepository], query: &str) -> Vec<&'a LocalRepository> {
    if query.is_empty() {
        // Return the all entries.
        return entries.iter().collect();
    }

    /// Check if entries contain the query characters with correct order.
    fn is_fuzzily_matched(s: &str, query: &str) -> bool {
        let mut iter = s.chars();
        for q in query.chars() {
            loop {
                match iter.next() {
                    None => return false,
                    Some(c) if c == q => break,
                    Some(_) => {}
                }
            }
        }
        true
    }

    // Filter entries by the query.
    let mut filtered = entries
        .iter()
        .filter(|repo| is_fuzzily_matched(&repo.relpath, query))
        .collect::<Vec<_>>();
    filtered.sort_by_cached_key(|entry| compute_score(&entry.relpath, query));
    filtered
}

/// Computes the similarity. Lower is more similar.
fn compute_score(entry: &str, query: &str) -> u8 {
    let mut score = u8::MAX;

    if entry == query {
        score -= 100;
    }

    if entry.starts_with(query) {
        score -= 10;
    }

    score
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::canonicalize;

    #[test]
    fn test_compute_score() {
        assert!(compute_score("abc", "abc") < compute_score("abcd", "abc"));
        assert!(compute_score("abcd", "abc") < compute_score("axbxc", "abc"));
        assert!(compute_score("abc", "a") < compute_score("abc", "b"));
    }

    #[test]
    fn test_fuzzy_search() {
        let backend = VCSBackend::GitBackend;
        let repos = vec![
            LocalRepository {
                path: "/path/to/repo1".to_string(),
                relpath: "repo1".to_string(),
                backend: backend.clone(),
            },
            LocalRepository {
                path: "/path/to/foo/repo2".to_string(),
                relpath: "foo/repo1".to_string(),
                backend: backend.clone(),
            },
            LocalRepository {
                path: "/path/to/bar/repo3".to_string(),
                relpath: "bar/repo3".to_string(),
                backend: backend.clone(),
            },
        ];
        let fuzzy = FuzzyVec::from_vec(repos);

        // Empty query returns all
        assert_eq!(fuzzy.search("").len(), 3);

        // Exact match
        let results = fuzzy.search("repo3");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].relpath, "bar/repo3");

        // Fuzzy match
        let results = fuzzy.search("r1");
        assert_eq!(results.len(), 2);

        // Correct order (relpath: "repo1" should be higher score than "foo/repo1")
        let results = fuzzy.search("repo1");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].relpath, "repo1");
    }

    #[test]
    fn read_dir() {
        let _ = env_logger::try_init();
        let root_path = "/home/ma2/repos";
        if let Ok(root_path) = canonicalize(root_path) {
            let mut result: Vec<LocalRepository> = vec![];
            walk_repository(root_path.to_str().unwrap(), &mut result).unwrap();
            println!("repos: {result:?}");
        }
    }

    #[test]
    fn test_walk_repository() {
        use std::fs::create_dir_all;
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();

        let repo1 = root.join("repo1");
        create_dir_all(repo1.join(".git")).unwrap();

        let sub = root.join("sub");
        let repo2 = sub.join("repo2");
        create_dir_all(repo2.join(".git")).unwrap();

        let mut repos = Vec::new();
        walk_repository(root.to_str().unwrap(), &mut repos).unwrap();

        assert_eq!(repos.len(), 2);
        let paths: Vec<String> = repos.iter().map(|r| r.relpath.clone()).collect();
        assert!(paths.contains(&"repo1".to_string()));
        assert!(paths.contains(&"sub/repo2".to_string()));
    }

    #[test]
    fn test_walk_repository_prunes_nested_repositories() {
        use std::fs::create_dir_all;
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();

        create_dir_all(root.join("repo1/.git")).unwrap();
        create_dir_all(root.join("repo1/nested/.git")).unwrap();
        create_dir_all(root.join("outside/repo2/.git")).unwrap();

        let mut repos = Vec::new();
        walk_repository(root.to_str().unwrap(), &mut repos).unwrap();

        let paths: Vec<String> = repos.iter().map(|r| r.relpath.clone()).collect();
        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&"repo1".to_string()));
        assert!(paths.contains(&"outside/repo2".to_string()));
        assert!(!paths.contains(&"repo1/nested".to_string()));
    }

    #[test]
    fn test_walk_repository_skips_hidden_directories() {
        use std::fs::create_dir_all;
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();

        create_dir_all(root.join(".cache/repo_hidden/.git")).unwrap();
        create_dir_all(root.join("visible/repo1/.git")).unwrap();
        create_dir_all(root.join("visible/.tmp/repo_hidden2/.git")).unwrap();

        let mut repos = Vec::new();
        walk_repository(root.to_str().unwrap(), &mut repos).unwrap();

        let paths: Vec<String> = repos.iter().map(|r| r.relpath.clone()).collect();
        assert_eq!(paths.len(), 1);
        assert!(paths.contains(&"visible/repo1".to_string()));
        assert!(!paths.contains(&".cache/repo_hidden".to_string()));
        assert!(!paths.contains(&"visible/.tmp/repo_hidden2".to_string()));
    }
}
