use crate::config::Config;
use crate::utils::{chdir, confirm, expand_home, output_with_work_dir, run_with_work_dir};
use crate::vcs::{VCSBackend, detect_vcs_from_path};
use anyhow::{Result, bail};
use log::{debug, error};
use rayon::prelude::*;
use std::collections::{BTreeSet, HashMap};
use std::fmt::{self, Debug, Formatter};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

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

fn walk_repository(root: &Path, repos: &mut Vec<LocalRepository>) {
    if let Err(e) = fs::metadata(root) {
        error!("{e} path:{root:?}");
        return;
    }
    repos.append(&mut scan_dir(root, root));
}

/// Recursively scans `dir` for repositories in parallel.
/// Once a directory is identified as a repository, its descendants are not scanned.
fn scan_dir(root: &Path, dir: &Path) -> Vec<LocalRepository> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            debug!("{e} path:{dir:?}");
            return vec![];
        }
    };

    let mut subdirs: Vec<PathBuf> = vec![];
    for entry in entries.flatten() {
        let name = entry.file_name();
        if let Some(backend) = name.to_str().and_then(detect_vcs_from_path) {
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
        // Symbolic links are not followed, and hidden directories are skipped.
        if !name.as_encoded_bytes().starts_with(b".") && entry.file_type().is_ok_and(|t| t.is_dir())
        {
            subdirs.push(entry.path());
        }
    }

    // Sort for a stable output order; `collect` keeps the order of `par_iter`.
    subdirs.sort_unstable();
    subdirs
        .par_iter()
        .flat_map_iter(|subdir| scan_dir(root, subdir))
        .collect()
}

/// Expands `~` in the given roots and removes duplicates.
fn expand_roots<'a>(roots: impl IntoIterator<Item = &'a String>) -> BTreeSet<PathBuf> {
    roots
        .into_iter()
        .filter_map(|root| {
            let expanded = expand_home(root);
            if expanded.is_none() {
                error!("failed to expand home. path:{root:?}");
            }
            expanded
        })
        .collect()
}

fn walk_repositories<'a>(roots: impl IntoIterator<Item = &'a String>) -> Vec<LocalRepository> {
    let mut result: Vec<LocalRepository> = vec![];
    for root in &expand_roots(roots) {
        walk_repository(root, &mut result);
    }
    dedup_repositories(result)
}

/// Removes repositories found under several nested roots. The shortest relative
/// path (relative to the innermost root) is kept, so that matching a query
/// behaves as if the repository was found only under that root.
fn dedup_repositories(repos: Vec<LocalRepository>) -> Vec<LocalRepository> {
    let mut index: HashMap<String, usize> = HashMap::with_capacity(repos.len());
    let mut result: Vec<LocalRepository> = Vec::with_capacity(repos.len());
    for repo in repos {
        match index.get(&repo.path) {
            Some(&i) => {
                if repo.relpath.len() < result[i].relpath.len() {
                    result[i].relpath = repo.relpath;
                }
            }
            None => {
                index.insert(repo.path.clone(), result.len());
                result.push(repo);
            }
        }
    }
    result
}

fn each_repo(
    config: &Config<'_>,
    f: impl FnOnce(&Config<'_>, &[&LocalRepository]) -> Result<()>,
) -> Result<()> {
    let repos = match config.profile {
        Some(profile) => walk_repositories([&config.profile(profile)?.root]),
        None => walk_repositories(config.roots()),
    };
    let fuzzy = FuzzyVec::from_vec(repos);
    let repos = fuzzy.search(&config.query);
    f(config, &repos)
}

/// Runs `run` for each repository in parallel, logging each failure.
/// Returns an error if `run` failed for any repository.
fn run_parallel(
    repos: &[&LocalRepository],
    label: &str,
    run: impl Fn(&LocalRepository) -> Result<()> + Sync,
) -> Result<()> {
    let failures = AtomicUsize::new(0);
    repos.par_iter().for_each(|repo| {
        if let Err(e) = run(repo) {
            failures.fetch_add(1, Ordering::Relaxed);
            error!("failed to {label} {}: {e:#}", repo.path);
        }
    });

    let failures = failures.into_inner();
    if failures > 0 {
        bail!(
            "{label} failed in {failures} of {} repositories",
            repos.len()
        );
    }
    Ok(())
}

/// Runs the update command of `repo` and prints its output as one block, so
/// that the outputs of updates running in parallel do not interleave.
fn update_repository(repo: &LocalRepository) -> Result<()> {
    let output = output_with_work_dir(repo.backend.update_command(), &repo.path);

    let mut stdout = io::stdout().lock();
    let mut stderr = io::stderr().lock();
    writeln!(stdout, "update {}", repo.path)?;
    let output = output?;
    stdout.write_all(&output.stdout)?;
    stdout.flush()?;
    stderr.write_all(&output.stderr)?;
    if !output.status.success() {
        bail!("{}", output.status);
    }
    Ok(())
}

pub fn list(config: &Config<'_>) -> Result<()> {
    each_repo(config, |_, repos| {
        let mut stdout = io::BufWriter::new(io::stdout().lock());
        for repo in repos {
            writeln!(stdout, "{}", repo.path)?;
        }
        stdout.flush()?;
        Ok(())
    })
}

pub fn update(config: &Config<'_>) -> Result<()> {
    each_repo(config, |_, repos| {
        run_parallel(repos, "update", update_repository)
    })
}

pub fn look(config: &Config<'_>) -> Result<()> {
    each_repo(config, |config, repos| match repos.first() {
        Some(repo) => chdir(&repo.path),
        None => bail!("{} not found", config.query),
    })
}

pub fn remove(config: &Config<'_>) -> Result<()> {
    each_repo(config, |config, repos| {
        if repos.is_empty() {
            bail!("{} not found", config.query);
        }
        for repo in repos {
            println!("{}", repo.path);
            if confirm("do you want to remove this? [y/N]", false)? {
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
        let Some(cmd) = config.each_cmd else {
            return Ok(());
        };
        if config.dry_run {
            for repo in repos {
                println!("{} : dry-run {:?} ", repo.path, cmd);
            }
            Ok(())
        } else {
            // Standard streams are inherited, so that the output is shown live
            // and interactive commands keep working.
            run_parallel(repos, "exec", |repo| {
                println!("{} : exec {:?} ", repo.path, cmd);
                run_with_work_dir(cmd, &repo.path)
            })
        }
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
                backend,
            },
            LocalRepository {
                path: "/path/to/foo/repo2".to_string(),
                relpath: "foo/repo1".to_string(),
                backend,
            },
            LocalRepository {
                path: "/path/to/bar/repo3".to_string(),
                relpath: "bar/repo3".to_string(),
                backend,
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
        walk_repository(root, &mut repos);

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
        walk_repository(root, &mut repos);

        let paths: Vec<String> = repos.iter().map(|r| r.relpath.clone()).collect();
        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&"repo1".to_string()));
        assert!(paths.contains(&"outside/repo2".to_string()));
        assert!(!paths.contains(&"repo1/nested".to_string()));
    }

    #[test]
    fn test_walk_repositories_nested_roots() {
        use std::fs::create_dir_all;
        let temp_dir = tempfile::tempdir().unwrap();
        let outer = temp_dir.path().join("src");
        let hidden = outer.join(".private");
        let work = outer.join("work");
        create_dir_all(outer.join("a/.git")).unwrap();
        create_dir_all(hidden.join("secret/.git")).unwrap();
        create_dir_all(work.join("proj/.git")).unwrap();

        let roots: Vec<String> = [&outer, &hidden, &work]
            .iter()
            .map(|p| p.to_str().unwrap().to_owned())
            .collect();
        let repos = walk_repositories(&roots);

        let mut relpaths: Vec<&str> = repos.iter().map(|r| r.relpath.as_str()).collect();
        relpaths.sort_unstable();
        // A hidden root is scanned, and a repository under nested roots is
        // listed once, relative to the innermost root.
        assert_eq!(relpaths, ["a", "proj", "secret"]);
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
        walk_repository(root, &mut repos);

        let paths: Vec<String> = repos.iter().map(|r| r.relpath.clone()).collect();
        assert_eq!(paths.len(), 1);
        assert!(paths.contains(&"visible/repo1".to_string()));
        assert!(!paths.contains(&".cache/repo_hidden".to_string()));
        assert!(!paths.contains(&"visible/.tmp/repo_hidden2".to_string()));
    }
}
