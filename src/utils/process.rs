use anyhow::{Result, bail};
use std::env;
use std::path::Path;
use std::process::{Command, ExitStatus, Output, Stdio};

fn command(cmd: &[&str]) -> Command {
    let (program, args) = cmd.split_first().expect("command must not be empty");
    let mut command = Command::new(program);
    command.args(args);
    command
}

fn check_status(cmd: &[&str], status: ExitStatus) -> Result<()> {
    if status.success() {
        Ok(())
    } else {
        bail!("`{}` failed ({status})", cmd.join(" "))
    }
}

/// Runs `cmd` with all standard streams discarded and reports whether it succeeded.
pub fn run_silently(cmd: &[&str]) -> Result<bool> {
    let status = command(cmd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;
    Ok(status.success())
}

/// Runs `cmd` with inherited standard streams. A non-zero exit status is an error.
pub fn run(cmd: &[&str]) -> Result<()> {
    let status = command(cmd).status()?;
    check_status(cmd, status)
}

/// Runs `cmd` in `dir` with inherited standard streams. A non-zero exit status is an error.
pub fn run_with_work_dir(cmd: &[&str], dir: impl AsRef<Path>) -> Result<()> {
    let status = command(cmd).current_dir(dir).status()?;
    check_status(cmd, status)
}

/// Runs `cmd` in `dir` and captures its output, so that commands running in
/// parallel do not interleave their output. Stdin is closed.
pub fn output_with_work_dir(cmd: &[&str], dir: impl AsRef<Path>) -> Result<Output> {
    Ok(command(cmd)
        .current_dir(dir)
        .stdin(Stdio::null())
        .output()?)
}

/// Spawns an interactive shell (`$SHELL`, or `/bin/sh`) in `dir`.
/// The exit status of the shell is not treated as an error.
pub fn chdir(dir: impl AsRef<Path>) -> Result<()> {
    let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_owned());
    command(&[&shell]).current_dir(dir).status()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_silently_success() {
        assert!(run_silently(&["true"]).unwrap());
        assert!(!run_silently(&["false"]).unwrap());
    }

    #[test]
    fn run_silently_missing_program() {
        assert!(run_silently(&["laaaaaas", "-al"]).is_err());
    }

    #[test]
    fn run_reports_exit_status() {
        run(&["true"]).unwrap();
        let err = run(&["false"]).unwrap_err();
        assert!(err.to_string().contains("`false` failed"), "{err}");
    }

    #[test]
    fn run_with_work_dir_uses_dir() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("marker"), "").unwrap();
        run_with_work_dir(&["test", "-f", "marker"], dir.path()).unwrap();
        assert!(run_with_work_dir(&["test", "-f", "missing"], dir.path()).is_err());
    }

    #[test]
    fn output_with_work_dir_captures() {
        let dir = tempfile::tempdir().unwrap();
        let out = output_with_work_dir(&["pwd"], dir.path()).unwrap();
        assert!(out.status.success());
        let pwd = String::from_utf8(out.stdout).unwrap();
        let expected = dir.path().canonicalize().unwrap();
        assert_eq!(Path::new(pwd.trim()).canonicalize().unwrap(), expected);
    }
}
