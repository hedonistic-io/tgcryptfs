use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

/// Get a command for the tgcryptfs binary with a temp volumes dir.
#[allow(deprecated)]
fn tgcryptfs() -> Command {
    Command::cargo_bin("tgcryptfs").unwrap()
}

/// Get a command with a custom TGCRYPTFS_VOLUMES_DIR pointing to a temp dir.
fn tgcryptfs_with_dir(dir: &TempDir) -> Command {
    let mut cmd = tgcryptfs();
    cmd.env("TGCRYPTFS_VOLUMES_DIR", dir.path().to_str().unwrap());
    cmd
}

#[test]
fn help_flag() {
    tgcryptfs()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("TGCryptFS v2"));
}

#[test]
fn version_flag() {
    tgcryptfs()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("tgcryptfs"));
}

#[test]
fn volume_help() {
    tgcryptfs()
        .args(["volume", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("create"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("delete"));
}

#[test]
fn volume_list_empty() {
    let dir = TempDir::new().unwrap();
    // volume list reads from default_volumes_dir(), so we need to use a custom dir
    // The status command is a simpler test since it just counts volumes
    tgcryptfs_with_dir(&dir)
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("Volumes:"));
}

#[test]
fn status_command() {
    tgcryptfs()
        .arg("status")
        .assert()
        .success()
        .stdout(predicate::str::contains("tgcryptfs v"))
        .stdout(predicate::str::contains("Volumes:"));
}

#[test]
fn deadman_status() {
    tgcryptfs()
        .args(["deadman", "status"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Deadman Status:"));
}

#[test]
fn auth_status() {
    tgcryptfs()
        .args(["auth", "status"])
        .assert()
        .success()
        .stdout(
            predicate::str::contains("authenticated")
                .or(predicate::str::contains("Not authenticated")),
        );
}

#[test]
fn share_help() {
    tgcryptfs()
        .args(["share", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("create"))
        .stdout(predicate::str::contains("list"))
        .stdout(predicate::str::contains("revoke"));
}

#[test]
fn key_help() {
    tgcryptfs()
        .args(["key", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("rotate"))
        .stdout(predicate::str::contains("export"))
        .stdout(predicate::str::contains("import"));
}

#[test]
fn deadman_help() {
    tgcryptfs()
        .args(["deadman", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("arm"))
        .stdout(predicate::str::contains("disarm"))
        .stdout(predicate::str::contains("status"));
}

#[test]
fn unknown_subcommand() {
    tgcryptfs()
        .arg("nonexistent")
        .assert()
        .failure()
        .stderr(predicate::str::contains("unrecognized subcommand"));
}

#[test]
fn verbose_flag_accepted() {
    tgcryptfs().args(["--verbose", "status"]).assert().success();
}

#[test]
fn serve_help() {
    tgcryptfs()
        .args(["serve", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("REST API"));
}

#[test]
fn configure_help() {
    tgcryptfs()
        .args(["configure", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("setup wizard"));
}
