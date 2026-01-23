use std::fs;

use std::process::Command;

#[test]
fn test_list_command() {
    let test_root = "tests/test_env";
    let _ = fs::remove_dir_all(test_root);
    fs::create_dir_all(format!("{}/repo1/.git", test_root)).unwrap();
    fs::create_dir_all(format!("{}/category/repo2/.git", test_root)).unwrap();
    fs::create_dir_all(format!("{}/not_a_repo", test_root)).unwrap();

    let config_path = "tests/integration_config.toml";
    fs::write(
        config_path,
        format!(
            r#"
[default]
root = "{}"
"#,
            test_root
        ),
    )
    .unwrap();

    let status = Command::new("cargo")
        .args(["run", "--", "-c", config_path, "list"])
        .output()
        .expect("Failed to execute command");

    assert!(status.status.success());
    let output = String::from_utf8(status.stdout).unwrap();

    assert!(output.contains(&format!("{}/repo1", test_root)));
    assert!(output.contains(&format!("{}/category/repo2", test_root)));
    assert!(!output.contains("not_a_repo")); // Should not list non-repos

    let _ = fs::remove_dir_all(test_root);
    let _ = fs::remove_file(config_path);
}
