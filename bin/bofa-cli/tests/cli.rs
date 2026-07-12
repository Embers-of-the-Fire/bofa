use std::process::Command as StdCommand;

#[test]
fn config_command_prints_config() {
    let path = std::env::temp_dir().join(format!(
        "bofa_test_config_command_{}.toml",
        std::process::id()
    ));
    std::fs::write(
        &path,
        "[credentials]\ntype = \"app\"\napp_id = \"$APP_ID\"\nkey_type = \"DER\"\nkey = \"$APP_KEY\"\n\n[repository]\nowner = \"owner\"\nrepo = \"repo\"\n\n[scanner.sensitive]\nenabled = true\n",
    )
    .unwrap();

    let output = StdCommand::new(snapbox::cmd::cargo_bin!("bofa"))
        .arg("--config")
        .arg(&path)
        .arg("config")
        .output()
        .unwrap();

    std::fs::remove_file(&path).unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("BofaConfig"));
    assert!(stdout.contains("SensitiveScannerConfig"));
}

#[test]
fn dry_run_flag_sets_worker_dry_run() {
    let path = std::env::temp_dir().join(format!(
        "bofa_test_config_dry_run_{}.toml",
        std::process::id()
    ));
    std::fs::write(
        &path,
        "[credentials]\ntype = \"app\"\napp_id = \"$APP_ID\"\nkey_type = \"DER\"\nkey = \"$APP_KEY\"\n\n[repository]\nowner = \"owner\"\nrepo = \"repo\"\n",
    )
    .unwrap();

    let output = StdCommand::new(snapbox::cmd::cargo_bin!("bofa"))
        .arg("--config")
        .arg(&path)
        .arg("--dry-run")
        .arg("config")
        .output()
        .unwrap();

    std::fs::remove_file(&path).unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("dry_run: true"));
}
