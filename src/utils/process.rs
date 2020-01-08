use anyhow::Result;
use std::env;
use std::process::{Command, Stdio};

pub fn run_silently(cmd: &[&str]) -> Result<bool> {
    let mut cmd = Command::new(&cmd[0])
        .args(&cmd[1..])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    let status = cmd.wait()?;
    Ok(status.success())
}

pub fn run(cmd: &[&str]) -> Result<bool> {
    let mut cmd = Command::new(&cmd[0])
        .args(&cmd[1..])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;
    let status = cmd.wait()?;
    Ok(status.success())
}

pub fn run_with_work_dir(cmd: &[&str], dir: &str) -> Result<bool> {
    let mut cmd = Command::new(&cmd[0])
        .args(&cmd[1..])
        .current_dir(dir)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()?;
    let status = cmd.wait()?;
    Ok(status.success())
}

pub fn chdir(dir: &str) -> Result<bool> {
    if let Ok(shell) = env::var("SHELL") {
        run_with_work_dir(&[&shell], dir)
    } else {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_silently1() {
        run_silently(&["ls", "-al"]).unwrap();
    }

    #[test]
    #[should_panic(expected = "No such file or directory")]
    fn run_silently2() {
        run_silently(&["laaaaaas", "-al"]).unwrap();
    }

    #[test]
    fn run1() {
        run(&["ls", "-al"]).unwrap();
    }
}
