use crate::config::Config;
use crate::utils::{chdir, confirm, run_with_work_dir};
use crate::vcs::{detect_vcs_from_path, VCSBackend, VCSOption};
use anyhow::Result;
use jwalk::WalkDir;
use log::error;
use rayon::prelude::*;
use std::fmt::{self, Debug, Formatter};
use std::fs;
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct LocalRepository {
    pub path: String,
    pub relpath: String,
    pub backend: VCSBackend,
}

impl LocalRepository {
    pub fn as_str(&self) -> &str {
        self.path.as_ref()
    }
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
    if let Err(e) = fs::metadata(root_path) {
        error!("{} path:{:?}", e, root_path);
        return Ok(());
    }

    let repos_mutex = Arc::new(Mutex::new(Vec::new()));
    let root_path_string = root_path.to_owned();
    let repos_clone = repos_mutex.clone();

    WalkDir::new(root_path)
        .skip_hidden(false)
        .process_read_dir(move |_depth, path, _state, children| {
            let mut found_backend = None;
            for child in children.iter().flatten() {
                if let Some(name) = child.file_name().to_str() {
                    if let Some(backend) = detect_vcs_from_path(name) {
                        found_backend = Some(backend);
                        break;
                    }
                }
            }

            if let Some(backend) = found_backend {
                if let Ok(mut g) = repos_clone.lock() {
                    // Use std::path::Path to calculate relative path safely
                    if let Ok(rel) = path.strip_prefix(&root_path_string) {
                        if let Some(rel_str) = rel.to_str() {
                            g.push(LocalRepository {
                                path: path.to_str().unwrap_or("").to_owned(),
                                relpath: rel_str.to_owned(),
                                backend,
                            });
                        }
                    }
                }

                // Prune VCS directories to avoid recursion
                children.retain(|c| {
                    if let Ok(child) = c {
                        if let Some(name) = child.file_name().to_str() {
                            if detect_vcs_from_path(name).is_some() {
                                return false;
                            }
                        }
                    }
                    true
                });
            }
        })
        .into_iter()
        .for_each(|_| {});

    let mut found = repos_mutex.lock().unwrap();
    repos.append(&mut found);
    Ok(())
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
                host: None,
            };
            println!("update {}", &opt.path);
            if let Err(e) = repo.backend.update(&opt) {
                error!("Failed to update {}: {}", &repo.path, e);
            }
        });
        Ok(())
    })
}

pub fn look(config: &Config<'_>) -> Result<()> {
    each_repo(config, |config, repos| {
        if repos.is_empty() {
            Err(anyhow::format_err!("{} not found", &config.query))
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
            println!("{}", &repo.path);
            if confirm("do you want to remove this? [Y/n]", "Y", Some("Y"))? {
                fs::remove_dir_all(&repo.path)?;
                println!("removed {}", &repo.path);
            }
            println!();
        }
        Ok(())
    })
}

pub fn each_exec(config: &Config<'_>) -> Result<()> {
    each_repo(config, |config, repos| {
        if let Some(cmd) = config.each_cmd {
            let cmd_owned: Vec<String> = cmd.iter().map(|s| s.to_string()).collect();
            if config.dry_run {
                for repo in repos {
                    println!("{} : dry-run {:?} ", &repo.path, &cmd);
                }
            } else {
                repos.par_iter().for_each(|repo| {
                    let cmd_refs: Vec<&str> = cmd_owned.iter().map(|s| s.as_str()).collect();
                    println!("{} : exec {:?} ", &repo.path, &cmd_refs);
                    if let Err(e) = run_with_work_dir(&cmd_refs, &repo.path) {
                        error!("Failed to exec in {}: {}", &repo.path, e);
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
    /// Creates a `FuzzyVec`.
    pub fn new() -> FuzzyVec {
        FuzzyVec {
            entries: Vec::new(),
        }
    }

    /// Creates a `FuzzyVec` from `entries`.
    pub fn from_vec(entries: Vec<LocalRepository>) -> FuzzyVec {
        FuzzyVec { entries }
    }

    /// Returns the number of entiries.
    #[inline]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    // Clears the contents.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// appends a entry.
    pub fn append(&mut self, entry: LocalRepository) {
        self.entries.push(entry);
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
    let mut score = std::u8::MAX;

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
    fn read_dir() {
        env_logger::try_init();
        let root_path = "/home/ma2/repos";
        if let Ok(root_path) = canonicalize(root_path) {
            let mut result: Vec<LocalRepository> = vec![];
            walk_repository(root_path.to_str().unwrap(), &mut result).unwrap();
            println!("repos: {:?}", result);
        }
    }
}
