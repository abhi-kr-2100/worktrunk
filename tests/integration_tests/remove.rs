use crate::common::{TestRepo, make_snapshot_cmd, setup_snapshot_settings};
use insta_cmd::assert_cmd_snapshot;
use std::process::Command;

/// Helper to create snapshot with normalized paths
fn snapshot_remove(test_name: &str, repo: &TestRepo, args: &[&str], cwd: Option<&std::path::Path>) {
    let settings = setup_snapshot_settings(repo);
    settings.bind(|| {
        let mut cmd = make_snapshot_cmd(repo, "remove", args, cwd);
        assert_cmd_snapshot!(test_name, cmd);
    });
}

#[test]
fn test_remove_already_on_default() {
    let mut repo = TestRepo::new();
    repo.commit("Initial commit");
    repo.setup_remote("main");

    // Already on main branch
    snapshot_remove("remove_already_on_default", &repo, &[], None);
}

#[test]
fn test_remove_switch_to_default() {
    let mut repo = TestRepo::new();
    repo.commit("Initial commit");
    repo.setup_remote("main");

    // Create and switch to a feature branch in the main repo
    let mut cmd = Command::new("git");
    repo.configure_git_cmd(&mut cmd);
    cmd.args(["switch", "-c", "feature"])
        .current_dir(repo.root_path())
        .output()
        .expect("Failed to create branch");

    snapshot_remove("remove_switch_to_default", &repo, &[], None);
}

#[test]
fn test_remove_from_worktree() {
    let mut repo = TestRepo::new();
    repo.commit("Initial commit");
    repo.setup_remote("main");

    let worktree_path = repo.add_worktree("feature-wt", "feature-wt");

    // Run remove from within the worktree
    snapshot_remove("remove_from_worktree", &repo, &[], Some(&worktree_path));
}

#[test]
fn test_remove_internal_mode() {
    let mut repo = TestRepo::new();
    repo.commit("Initial commit");
    repo.setup_remote("main");

    let worktree_path = repo.add_worktree("feature-internal", "feature-internal");

    snapshot_remove(
        "remove_internal_mode",
        &repo,
        &["--internal"],
        Some(&worktree_path),
    );
}

#[test]
fn test_remove_dirty_working_tree() {
    let mut repo = TestRepo::new();
    repo.commit("Initial commit");
    repo.setup_remote("main");

    // Create a dirty file
    std::fs::write(repo.root_path().join("dirty.txt"), "uncommitted changes")
        .expect("Failed to create file");

    snapshot_remove("remove_dirty_working_tree", &repo, &[], None);
}
