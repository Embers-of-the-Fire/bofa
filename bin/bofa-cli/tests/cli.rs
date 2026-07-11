use snapbox::cmd::{Command, cargo_bin};

#[test]
fn hello_world() {
    Command::new(cargo_bin!("bofa"))
        .arg("hello")
        .assert()
        .success()
        .stdout_eq("Hello, world!\n");
}
