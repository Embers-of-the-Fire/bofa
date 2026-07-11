use snapbox::cmd::Command;
use std::process::Command as StdCommand;

#[test]
fn hello_world() {
    Command::new(snapbox::cmd::cargo_bin!("bofa"))
        .arg("hello")
        .assert()
        .success()
        .stdout_eq("Hello, world!\n");
}

#[test]
fn config_command_prints_config() {
    let path = std::env::temp_dir().join(format!("bofa_test_config_{}.toml", std::process::id()));
    std::fs::write(
        &path,
        "[credentials]\ntype = \"app\"\napp_id = \"$APP_ID\"\nkey_type = \"DER\"\nkey = \"$APP_KEY\"\n\n[scanner.sensitive]\nenabled = true\nitem = []\n",
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
