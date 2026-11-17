use std::fs;

use std::process::Command;

#[test]
fn test_list_command() {
    let temp_dir = tempfile::tempdir().unwrap();
    let test_root = temp_dir.path().join("test_env");
    let test_root = test_root.to_str().unwrap();
    fs::create_dir_all(format!("{test_root}/repo1/.git")).unwrap();
    fs::create_dir_all(format!("{test_root}/category/repo2/.git")).unwrap();
    fs::create_dir_all(format!("{test_root}/not_a_repo")).unwrap();

    let config_path = temp_dir.path().join("integration_config.toml");
    fs::write(
        &config_path,
        format!(
            r#"
[default]
root = "{test_root}"
"#
        ),
    )
    .unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_rrc"))
        .arg("-c")
        .arg(&config_path)
        .arg("list")
        .output()
        .expect("Failed to execute command");

    assert!(status.status.success());
    let output = String::from_utf8(status.stdout).unwrap();

    assert!(output.contains(&format!("{test_root}/repo1")));
    assert!(output.contains(&format!("{test_root}/category/repo2")));
    assert!(!output.contains("not_a_repo")); // Should not list non-repos
}

use std::path::{Path, PathBuf};
use std::process::{Output, Stdio};

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(["-c", "user.name=test", "-c", "user.email=test@example.com"])
        .args(args)
        .current_dir(dir)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

/// A temporary root with a config file pointing to `<tmp>/root`.
struct Env {
    _temp: tempfile::TempDir,
    base: PathBuf,
    root: PathBuf,
    config: PathBuf,
}

impl Env {
    fn new() -> Env {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path().to_path_buf();
        let root = base.join("root");
        fs::create_dir_all(&root).unwrap();
        let config = base.join("config.toml");
        fs::write(
            &config,
            format!("[default]\nroot = {:?}\n", root.to_str().unwrap()),
        )
        .unwrap();
        Env {
            _temp: temp,
            base,
            root,
            config,
        }
    }

    /// Creates a bare upstream with one commit and clones it to `<root>/<relpath>`.
    fn clone_repo(&self, relpath: &str) -> PathBuf {
        let upstream = self.base.join("upstream").join(relpath);
        fs::create_dir_all(&upstream).unwrap();
        git(&upstream, &["init", "--bare", "-q"]);
        let work = self.base.join("work").join(relpath);
        fs::create_dir_all(&work).unwrap();
        git(&work, &["init", "-q"]);
        git(&work, &["commit", "-q", "--allow-empty", "-m", "init"]);
        git(
            &work,
            &[
                "push",
                "-q",
                upstream.to_str().unwrap(),
                "HEAD:refs/heads/main",
            ],
        );
        git(&upstream, &["symbolic-ref", "HEAD", "refs/heads/main"]);

        let dest = self.root.join(relpath);
        fs::create_dir_all(dest.parent().unwrap()).unwrap();
        git(
            &self.base,
            &[
                "clone",
                "-q",
                upstream.to_str().unwrap(),
                dest.to_str().unwrap(),
            ],
        );
        dest
    }

    fn rrc(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_rrc"))
            .arg("-c")
            .arg(&self.config)
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .stdin(Stdio::null())
            .output()
            .expect("Failed to execute command")
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn test_update_succeeds_and_reports_failures() {
    let env = Env::new();
    let ok = env.clone_repo("example.invalid/user/ok");
    let ok_output = env.rrc(&["update"]);
    assert!(ok_output.status.success(), "{ok_output:?}");
    assert!(stdout(&ok_output).contains(&format!("update {}", ok.display())));

    // A repository without an upstream cannot be pulled.
    let broken = env.root.join("example.invalid/user/broken");
    fs::create_dir_all(&broken).unwrap();
    git(&broken, &["init", "-q"]);

    let output = env.rrc(&["update"]);
    assert!(!output.status.success(), "update must fail: {output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("update failed in 1 of 2 repositories"),
        "{stderr}"
    );
}

#[test]
fn test_each_reports_failures() {
    let env = Env::new();
    fs::create_dir_all(env.root.join("a/.git")).unwrap();
    fs::create_dir_all(env.root.join("b/.git")).unwrap();

    let output = env.rrc(&["each", "--", "sh", "-c", "echo hello-from-each"]);
    assert!(output.status.success(), "{output:?}");
    let out = stdout(&output);
    assert_eq!(out.lines().filter(|l| *l == "hello-from-each").count(), 2);

    let output = env.rrc(&["each", "-e", "a", "--", "false"]);
    assert!(!output.status.success(), "{output:?}");
}

#[test]
fn test_remove_without_input_keeps_repositories() {
    let env = Env::new();
    let repo = env.root.join("example.invalid/user/repo");
    fs::create_dir_all(repo.join(".git")).unwrap();

    let output = env.rrc(&["remove", "repo"]);
    assert!(output.status.success(), "{output:?}");
    assert!(repo.exists(), "closed stdin must not confirm removal");

    let output = env.rrc(&["remove", "no-such-repository"]);
    assert!(!output.status.success(), "{output:?}");
}

#[test]
fn test_get_existing_repository_is_skipped() {
    let env = Env::new();
    let repo = env.root.join("example.invalid/user/repo");
    fs::create_dir_all(repo.join(".git")).unwrap();

    // No network access happens: the clone exists locally.
    let output = env.rrc(&["get", "git@example.invalid:user/repo.git"]);
    assert!(output.status.success(), "{output:?}");
    assert!(stdout(&output).contains(&format!("exists {}", repo.display())));
}

#[test]
fn test_get_update_uses_local_clone() {
    let env = Env::new();
    let repo = env.clone_repo("example.invalid/user/repo");

    let output = env.rrc(&["get", "-u", "https://example.invalid/user/repo"]);
    assert!(output.status.success(), "{output:?}");
    assert!(stdout(&output).contains(&format!("update {}", repo.display())));
}
