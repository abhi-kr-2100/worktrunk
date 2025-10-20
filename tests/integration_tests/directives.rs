use crate::common::TestRepo;
use insta::Settings;
use insta_cmd::{assert_cmd_snapshot, get_cargo_bin};
use std::process::Command;

/// Test the directive protocol for switch command
#[test]
fn test_switch_internal_directive() {
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    let mut settings = Settings::clone_current();
    settings.set_snapshot_path("../snapshots");

    // Normalize the directive path output
    settings.add_filter(r"__WORKTRUNK_CD__[^\n]+", "__WORKTRUNK_CD__[PATH]");

    settings.bind(|| {
        let mut cmd = Command::new(get_cargo_bin("wt"));
        repo.clean_cli_env(&mut cmd);
        cmd.arg("switch")
            .arg("my-feature")
            .arg("--internal")
            .current_dir(repo.root_path());

        assert_cmd_snapshot!("switch_internal_directive", cmd);
    });
}

/// Test switch without internal flag (should show help message)
#[test]
fn test_switch_without_internal() {
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    let mut settings = Settings::clone_current();
    settings.set_snapshot_path("../snapshots");

    settings.bind(|| {
        let mut cmd = Command::new(get_cargo_bin("wt"));
        repo.clean_cli_env(&mut cmd);
        cmd.arg("switch")
            .arg("my-feature")
            .current_dir(repo.root_path());

        assert_cmd_snapshot!("switch_without_internal", cmd);
    });
}

/// Test remove command with internal flag
#[test]
fn test_remove_internal_directive() {
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    let mut settings = Settings::clone_current();
    settings.set_snapshot_path("../snapshots");

    // Normalize the directive path output
    settings.add_filter(r"__WORKTRUNK_CD__[^\n]+", "__WORKTRUNK_CD__[PATH]");

    settings.bind(|| {
        let mut cmd = Command::new(get_cargo_bin("wt"));
        repo.clean_cli_env(&mut cmd);
        cmd.arg("remove")
            .arg("--internal")
            .current_dir(repo.root_path());

        assert_cmd_snapshot!("remove_internal_directive", cmd);
    });
}

/// Test remove without internal flag
#[test]
fn test_remove_without_internal() {
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    let mut settings = Settings::clone_current();
    settings.set_snapshot_path("../snapshots");

    settings.bind(|| {
        let mut cmd = Command::new(get_cargo_bin("wt"));
        repo.clean_cli_env(&mut cmd);
        cmd.arg("remove").current_dir(repo.root_path());

        assert_cmd_snapshot!("remove_without_internal", cmd);
    });
}
