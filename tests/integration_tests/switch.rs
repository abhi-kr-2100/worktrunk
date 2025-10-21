use crate::common::{TestRepo, make_snapshot_cmd, setup_snapshot_settings};
use insta_cmd::assert_cmd_snapshot;

/// Helper to create snapshot with normalized paths and SHAs
fn snapshot_switch(test_name: &str, repo: &TestRepo, args: &[&str]) {
    let settings = setup_snapshot_settings(repo);
    settings.bind(|| {
        let mut cmd = make_snapshot_cmd(repo, "switch", args, None);
        assert_cmd_snapshot!(test_name, cmd);
    });
}

#[test]
fn test_switch_create_new_branch() {
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    snapshot_switch("switch_create_new", &repo, &["--create", "feature-x"]);
}

#[test]
fn test_switch_create_existing_branch_error() {
    let mut repo = TestRepo::new();
    repo.commit("Initial commit");

    // Create a branch first
    repo.add_worktree("feature-y", "feature-y");

    // Try to create it again - should error
    snapshot_switch(
        "switch_create_existing_error",
        &repo,
        &["--create", "feature-y"],
    );
}

#[test]
fn test_switch_existing_branch() {
    let mut repo = TestRepo::new();
    repo.commit("Initial commit");

    // Create a worktree for a branch
    repo.add_worktree("feature-z", "feature-z");

    // Switch to it (should find existing worktree)
    snapshot_switch("switch_existing_branch", &repo, &["feature-z"]);
}

#[test]
fn test_switch_with_base_branch() {
    let repo = TestRepo::new();
    repo.commit("Initial commit on main");

    snapshot_switch(
        "switch_with_base",
        &repo,
        &["--create", "--base", "main", "feature-with-base"],
    );
}

#[test]
fn test_switch_base_without_create_warning() {
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    snapshot_switch(
        "switch_base_without_create",
        &repo,
        &["--base", "main", "main"],
    );
}

#[test]
fn test_switch_internal_mode() {
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    snapshot_switch(
        "switch_internal_mode",
        &repo,
        &["--create", "--internal", "internal-test"],
    );
}

#[test]
fn test_switch_existing_worktree_internal() {
    let mut repo = TestRepo::new();
    repo.commit("Initial commit");

    repo.add_worktree("existing-wt", "existing-wt");

    snapshot_switch(
        "switch_existing_internal",
        &repo,
        &["--internal", "existing-wt"],
    );
}

#[test]
fn test_switch_error_missing_worktree_directory() {
    let mut repo = TestRepo::new();
    repo.commit("Initial commit");

    // Create a worktree
    let wt_path = repo.add_worktree("missing-wt", "missing-wt");

    // Remove the worktree directory (but leave it registered in git)
    std::fs::remove_dir_all(&wt_path).expect("Failed to remove worktree directory");

    // Try to switch to the missing worktree (should fail)
    snapshot_switch("switch_error_missing_directory", &repo, &["missing-wt"]);
}

#[test]
fn test_switch_execute_success() {
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    snapshot_switch(
        "switch_execute_success",
        &repo,
        &["--create", "exec-test", "--execute", "echo 'test output'"],
    );
}

#[test]
fn test_switch_execute_creates_file() {
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    // Use platform-specific command
    #[cfg(target_os = "windows")]
    let create_file_cmd = "echo test > test.txt";
    #[cfg(not(target_os = "windows"))]
    let create_file_cmd = "echo 'test content' > test.txt";

    snapshot_switch(
        "switch_execute_creates_file",
        &repo,
        &["--create", "file-test", "--execute", create_file_cmd],
    );
}

#[test]
fn test_switch_execute_failure() {
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    snapshot_switch(
        "switch_execute_failure",
        &repo,
        &["--create", "fail-test", "--execute", "exit 1"],
    );
}

#[test]
fn test_switch_execute_with_existing_worktree() {
    let mut repo = TestRepo::new();
    repo.commit("Initial commit");

    // Create a worktree first
    repo.add_worktree("existing-exec", "existing-exec");

    // Use platform-specific command
    #[cfg(target_os = "windows")]
    let create_file_cmd = "echo existing > existing.txt";
    #[cfg(not(target_os = "windows"))]
    let create_file_cmd = "echo 'existing worktree' > existing.txt";

    snapshot_switch(
        "switch_execute_existing",
        &repo,
        &["existing-exec", "--execute", create_file_cmd],
    );
}

#[test]
fn test_switch_execute_with_internal_error() {
    let repo = TestRepo::new();
    repo.commit("Initial commit");

    snapshot_switch(
        "switch_execute_internal_error",
        &repo,
        &[
            "--create",
            "internal-test",
            "--execute",
            "echo test",
            "--internal",
        ],
    );
}
