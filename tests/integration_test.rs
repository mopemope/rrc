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
