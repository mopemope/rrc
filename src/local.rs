use crate::config::Config;
use crate::utils::{chdir, confirm};
use crate::vcs::{detect_vcs_from_path, VCSBackend, VCSOption};
use anyhow::{Context, Result};
use async_std::task;
use std::fmt::{self, Debug, Formatter};
use std::fs;
use std::path::{Path, PathBuf};
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

fn find_repository(
    root_path: Arc<String>,
    path: &PathBuf,
    entry: fs::DirEntry,
) -> Result<Option<LocalRepository>> {
    if let Some(file_name) = entry.file_name().to_str() {
        if let Some(backend) = detect_vcs_from_path(file_name) {
            let path = fs::canonicalize(&path)?;
            let path = path
                .parent()
                .context("failed get parent")?
                .to_str()
                .context("failed get parent")?
                .to_owned();
            let relpath = path[root_path.len() + 1..].to_owned();
            return Ok(Some(LocalRepository {
                path,
                relpath,
                backend,
            }));
        }
    }
    Ok(None)
}

fn find_sub_repositories(
    root_path: Arc<String>,
    root: &Path,
    repos: &mut Arc<Mutex<Vec<LocalRepository>>>,
) -> Result<()> {
    let root = fs::read_dir(root)?;
    for entry in root {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::metadata(&path)?;
        if metadata.is_file() {
            continue;
        }
        if let Some(repo) = find_repository(Arc::clone(&root_path), &path, entry)? {
            let mut repos = repos.lock().unwrap();
            repos.push(repo);
            return Ok(());
        } else {
            find_sub_repositories(Arc::clone(&root_path), &path, repos)?;
        }
    }
    Ok(())
}

async fn find_repositories(
    root_path: Arc<String>,
    root: &Path,
    repos: &mut Arc<Mutex<Vec<LocalRepository>>>,
) -> Result<()> {
    let root = fs::read_dir(root)?;
    for entry in root {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::metadata(&path)?;
        if metadata.is_file() {
            continue;
        }
        if let Some(repo) = find_repository(Arc::clone(&root_path), &path, entry)? {
            let mut repos = repos.lock().unwrap();
            repos.push(repo);
            return Ok(());
        } else {
            find_sub_repositories(Arc::clone(&root_path), &path, repos)?;
        }
    }

    Ok(())
}

async fn find_user_repositories(
    root_path: Arc<String>,
    root: &Path,
    repos: &mut Arc<Mutex<Vec<LocalRepository>>>,
) -> Result<()> {
    let root = fs::read_dir(root)?;
    for entry in root {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::metadata(&path)?;
        if metadata.is_file() {
            continue;
        }
        if let Some(repo) = find_repository(Arc::clone(&root_path), &path, entry)? {
            let mut repos = repos.lock().unwrap();
            repos.push(repo);
            return Ok(());
        }
        find_repositories(Arc::clone(&root_path), &path, repos).await?;
    }
    Ok(())
}

async fn find_service_repositories(
    root_path: Arc<String>,
    root: &Path,
    repos: &mut Arc<Mutex<Vec<LocalRepository>>>,
) -> Result<()> {
    let root = fs::read_dir(root)?;
    for entry in root {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::metadata(&path)?;
        if metadata.is_file() {
            continue;
        }
        find_user_repositories(Arc::clone(&root_path), &path, &mut Arc::clone(&repos)).await?;
    }

    Ok(())
}

fn walk_repository(root_path: &str, repos: &mut Arc<Mutex<Vec<LocalRepository>>>) -> Result<()> {
    let root = fs::read_dir(root_path)?;
    let root_path = Arc::new(root_path.to_owned());
    let mut futures: Vec<task::JoinHandle<Result<()>>> = vec![];

    for entry in root {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::metadata(&path)?;
        if metadata.is_file() {
            continue;
        }

        let repos = Arc::clone(&repos);
        let root_path = Arc::clone(&root_path);
        let f = task::spawn(async move {
            find_service_repositories(Arc::clone(&root_path), &path, &mut Arc::clone(&repos)).await
        });
        futures.push(f);
    }

    for f in futures {
        let _ = task::block_on(f)?;
    }

    Ok(())
}

fn walk_repositories(config: &Config<'_>) -> Result<Vec<LocalRepository>> {
    let result: Vec<LocalRepository> = vec![];
    let mut result: Arc<Mutex<Vec<LocalRepository>>> = Arc::new(Mutex::new(result));

    for root in config.roots() {
        walk_repository(root, &mut result)?;
    }
    let result = result.lock().unwrap().to_vec();
    Ok(result)
}

fn list_repos(config: &Config<'_>, profile: &str) -> Result<Vec<LocalRepository>> {
    let repo_config = config.profile(profile)?;
    let result: Vec<LocalRepository> = vec![];
    let mut result: Arc<Mutex<Vec<LocalRepository>>> = Arc::new(Mutex::new(result));
    walk_repository(&repo_config.root, &mut result)?;
    let result = result.lock().unwrap().to_vec();
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
        for repo in repos {
            let opt = VCSOption {
                url: None,
                path: repo.path.clone(),
                host: None,
            };
            println!("update {}", &opt.path);
            repo.backend.update(&opt)?;
            println!();
        }
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
        let root_path = "/home/ma2/repos";
        let root_path = canonicalize(root_path).unwrap();
        let result: Vec<LocalRepository> = vec![];
        let mut result: Arc<Mutex<Vec<LocalRepository>>> = Arc::new(Mutex::new(result));
        walk_repository(root_path.to_str().unwrap(), &mut result).unwrap();
        println!("repos: {:?}", result);
    }
}
