use std::{fs, process::Command};

fn command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_systemd-gtk"));
    command.env_remove("DISPLAY").env_remove("WAYLAND_DISPLAY");
    command
}

#[test]
fn help_version_and_invalid_options_do_not_require_a_display() {
    for flag in ["--help", "-h", "--version", "-V"] {
        let output = command().arg(flag).output().unwrap();
        assert!(output.status.success(), "{output:?}");
        assert!(String::from_utf8_lossy(&output.stdout).contains("systemd-gtk"));
    }
    assert!(
        !command()
            .arg("--unknown")
            .output()
            .unwrap()
            .status
            .success()
    );
    assert!(
        !command()
            .args(["--install-desktop", "unexpected"])
            .output()
            .unwrap()
            .status
            .success()
    );
}

#[test]
fn application_does_not_install_desktop_assets() {
    let temp = tempfile::tempdir().unwrap();
    let output = command()
        .arg("--install-desktop")
        .env("XDG_DATA_HOME", temp.path())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 0);
    let help = command().arg("--help").output().unwrap();
    assert!(!String::from_utf8_lossy(&help.stdout).contains("--install-desktop"));
}
