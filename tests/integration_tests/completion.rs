use crate::common::TestRepo;
use assert_cmd::Command;
use std::process::Command as StdCommand;

#[test]
fn test_complete_switch_shows_branches() {
    let temp = TestRepo::new();
    temp.commit("initial");

    // Create some branches using git
    StdCommand::new("git")
        .args(["branch", "feature/new"])
        .current_dir(temp.root_path())
        .output()
        .unwrap();

    StdCommand::new("git")
        .args(["branch", "hotfix/bug"])
        .current_dir(temp.root_path())
        .output()
        .unwrap();

    // Test completion for switch command
    let mut cmd = Command::cargo_bin("wt").unwrap();
    let output = cmd
        .current_dir(temp.root_path())
        .args(["complete", "wt", "switch", ""])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let branches: Vec<&str> = stdout.lines().collect();

    // Should include both branches (no worktrees created yet)
    assert!(branches.iter().any(|b| b.contains("feature/new")));
    assert!(branches.iter().any(|b| b.contains("hotfix/bug")));
}

#[test]
fn test_complete_switch_shows_all_branches_including_worktrees() {
    let mut temp = TestRepo::new();
    temp.commit("initial");

    // Create worktree (this creates a new branch "feature/new")
    temp.add_worktree("feature-worktree", "feature/new");

    // Create another branch without worktree
    StdCommand::new("git")
        .args(["branch", "hotfix/bug"])
        .current_dir(temp.root_path())
        .output()
        .unwrap();

    // Test completion - should show branches WITH worktrees and WITHOUT worktrees
    let mut cmd = Command::cargo_bin("wt").unwrap();
    let output = cmd
        .current_dir(temp.root_path())
        .args(["complete", "wt", "switch", ""])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let branches: Vec<&str> = stdout.lines().collect();

    // Should include feature/new (even though it has worktree - can switch to it)
    assert!(branches.iter().any(|b| b.contains("feature/new")));
    // Should include hotfix/bug (no worktree)
    assert!(branches.iter().any(|b| b.contains("hotfix/bug")));
}

#[test]
fn test_complete_push_shows_all_branches() {
    let mut temp = TestRepo::new();
    temp.commit("initial");

    // Create worktree (creates "feature/new" branch)
    temp.add_worktree("feature-worktree", "feature/new");

    // Create another branch without worktree
    StdCommand::new("git")
        .args(["branch", "hotfix/bug"])
        .current_dir(temp.root_path())
        .output()
        .unwrap();

    // Test completion for push (should show ALL branches, including those with worktrees)
    let mut cmd = Command::cargo_bin("wt").unwrap();
    let output = cmd
        .current_dir(temp.root_path())
        .args(["complete", "wt", "push", ""])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let branches: Vec<&str> = stdout.lines().collect();

    // Should include both branches (push shows all)
    assert!(branches.iter().any(|b| b.contains("feature/new")));
    assert!(branches.iter().any(|b| b.contains("hotfix/bug")));
}

#[test]
fn test_complete_base_flag_shows_all_branches() {
    let temp = TestRepo::new();
    temp.commit("initial");

    // Create branches
    StdCommand::new("git")
        .args(["branch", "develop"])
        .current_dir(temp.root_path())
        .output()
        .unwrap();

    StdCommand::new("git")
        .args(["branch", "feature/existing"])
        .current_dir(temp.root_path())
        .output()
        .unwrap();

    // Test completion for --base flag (long form)
    let mut cmd = Command::cargo_bin("wt").unwrap();
    let output = cmd
        .current_dir(temp.root_path())
        .args([
            "complete",
            "wt",
            "switch",
            "--create",
            "new-branch",
            "--base",
            "",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let branches: Vec<&str> = stdout.lines().collect();

    // Should show all branches as potential base
    assert!(branches.iter().any(|b| b.contains("develop")));
    assert!(branches.iter().any(|b| b.contains("feature/existing")));

    // Test completion for -b flag (short form)
    let mut cmd = Command::cargo_bin("wt").unwrap();
    let output = cmd
        .current_dir(temp.root_path())
        .args([
            "complete",
            "wt",
            "switch",
            "--create",
            "new-branch",
            "-b",
            "",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let branches: Vec<&str> = stdout.lines().collect();

    // Should show all branches as potential base (short form works too)
    assert!(branches.iter().any(|b| b.contains("develop")));
}

#[test]
fn test_complete_edge_cases_return_empty() {
    // Test 1: Outside git repo
    let temp = tempfile::tempdir().unwrap();
    let mut cmd = Command::cargo_bin("wt").unwrap();
    let output = cmd
        .current_dir(temp.path())
        .args(["complete", "wt", "switch", ""])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "");

    // Test 2: Empty repo (no commits)
    let repo = TestRepo::new();
    let mut cmd = Command::cargo_bin("wt").unwrap();
    let output = cmd
        .current_dir(repo.root_path())
        .args(["complete", "wt", "switch", ""])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "");

    // Test 3: Unknown command
    let repo = TestRepo::new();
    repo.commit("initial");
    let mut cmd = Command::cargo_bin("wt").unwrap();
    let output = cmd
        .current_dir(repo.root_path())
        .args(["complete", "wt", "unknown-command", ""])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "");

    // Test 4: Commands that don't take branch arguments (list)
    let mut cmd = Command::cargo_bin("wt").unwrap();
    let output = cmd
        .current_dir(repo.root_path())
        .args(["complete", "wt", "list", ""])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "");
}

#[test]
fn test_init_fish_includes_no_file_flag() {
    // Test that fish init includes -f flag to disable file completion
    let mut cmd = Command::cargo_bin("wt").unwrap();
    let output = cmd.arg("init").arg("fish").output().unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Check that completions include -f flag
    assert!(stdout.contains("-f -a '(__wt_complete)'"));
}

#[test]
fn test_complete_with_partial_prefix() {
    let temp = TestRepo::new();
    temp.commit("initial");

    // Create branches with common prefix
    StdCommand::new("git")
        .args(["branch", "feature/one"])
        .current_dir(temp.root_path())
        .output()
        .unwrap();

    StdCommand::new("git")
        .args(["branch", "feature/two"])
        .current_dir(temp.root_path())
        .output()
        .unwrap();

    StdCommand::new("git")
        .args(["branch", "hotfix/bug"])
        .current_dir(temp.root_path())
        .output()
        .unwrap();

    // Complete with partial prefix - should return all branches
    // (shell completion framework handles the prefix filtering)
    let mut cmd = Command::cargo_bin("wt").unwrap();
    let output = cmd
        .current_dir(temp.root_path())
        .args(["complete", "wt", "switch", "feat"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should return all branches, not just those matching "feat"
    // The shell will filter based on user input
    assert!(stdout.contains("feature/one"));
    assert!(stdout.contains("feature/two"));
    assert!(stdout.contains("hotfix/bug"));
}

#[test]
fn test_complete_switch_shows_all_branches_even_with_worktrees() {
    let mut temp = TestRepo::new();
    temp.commit("initial");

    // Create two branches, both with worktrees
    temp.add_worktree("feature-worktree", "feature/new");
    temp.add_worktree("hotfix-worktree", "hotfix/bug");

    // From the main worktree, test completion - should show all branches
    let mut cmd = Command::cargo_bin("wt").unwrap();
    let output = cmd
        .current_dir(temp.root_path())
        .args(["complete", "wt", "switch", ""])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should include branches even if they have worktrees (can switch to them)
    assert!(stdout.contains("feature/new"));
    assert!(stdout.contains("hotfix/bug"));
}

#[test]
fn test_complete_excludes_remote_branches() {
    let temp = TestRepo::new();
    temp.commit("initial");

    // Create local branches
    StdCommand::new("git")
        .args(["branch", "feature/local"])
        .current_dir(temp.root_path())
        .output()
        .unwrap();

    // Set up a fake remote
    StdCommand::new("git")
        .args(["remote", "add", "origin", "https://example.com/repo.git"])
        .current_dir(temp.root_path())
        .output()
        .unwrap();

    // Create a remote-tracking branch by fetching from a local "remote"
    // First, create a bare repo to act as remote
    let remote_dir = temp.root_path().parent().unwrap().join("remote.git");
    StdCommand::new("git")
        .args(["init", "--bare", remote_dir.to_str().unwrap()])
        .output()
        .unwrap();

    // Update remote URL to point to our bare repo
    StdCommand::new("git")
        .args(["remote", "set-url", "origin", remote_dir.to_str().unwrap()])
        .current_dir(temp.root_path())
        .output()
        .unwrap();

    // Push to create remote branches
    StdCommand::new("git")
        .args(["push", "origin", "main"])
        .current_dir(temp.root_path())
        .output()
        .unwrap();

    StdCommand::new("git")
        .args(["push", "origin", "feature/local:feature/remote"])
        .current_dir(temp.root_path())
        .output()
        .unwrap();

    // Fetch to create remote-tracking branches
    StdCommand::new("git")
        .args(["fetch", "origin"])
        .current_dir(temp.root_path())
        .output()
        .unwrap();

    // Test completion
    let mut cmd = Command::cargo_bin("wt").unwrap();
    let output = cmd
        .current_dir(temp.root_path())
        .args(["complete", "wt", "switch", ""])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should include local branch without worktree
    assert!(
        stdout.contains("feature/local"),
        "Should include feature/local branch, but got: {}",
        stdout
    );

    // main branch has a worktree (the root repo), so it may or may not be included
    // depending on switch context - not critical for this test

    // Should NOT include remote-tracking branches (origin/*)
    assert!(
        !stdout.contains("origin/"),
        "Completion should not include remote-tracking branches, but found: {}",
        stdout
    );
}

#[test]
fn test_complete_merge_shows_branches() {
    let mut temp = TestRepo::new();
    temp.commit("initial");

    // Create worktree (creates "feature/new" branch)
    temp.add_worktree("feature-worktree", "feature/new");

    // Create another branch without worktree
    StdCommand::new("git")
        .args(["branch", "hotfix/bug"])
        .current_dir(temp.root_path())
        .output()
        .unwrap();

    // Test completion for merge (should show ALL branches, including those with worktrees)
    let mut cmd = Command::cargo_bin("wt").unwrap();
    let output = cmd
        .current_dir(temp.root_path())
        .args(["complete", "wt", "merge", ""])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let branches: Vec<&str> = stdout.lines().collect();

    // Should include both branches (merge shows all)
    assert!(branches.iter().any(|b| b.contains("feature/new")));
    assert!(branches.iter().any(|b| b.contains("hotfix/bug")));
}

#[test]
fn test_complete_with_special_characters_in_branch_names() {
    let temp = TestRepo::new();
    temp.commit("initial");

    // Create branches with various special characters
    let branch_names = vec![
        "feature/FOO-123",         // Uppercase + dash + numbers
        "release/v1.2.3",          // Dots
        "hotfix/bug_fix",          // Underscore
        "feature/multi-part-name", // Multiple dashes
    ];

    for branch in &branch_names {
        StdCommand::new("git")
            .args(["branch", branch])
            .current_dir(temp.root_path())
            .output()
            .unwrap();
    }

    // Test completion
    let mut cmd = Command::cargo_bin("wt").unwrap();
    let output = cmd
        .current_dir(temp.root_path())
        .args(["complete", "wt", "switch", ""])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // All branches should be present
    for branch in &branch_names {
        assert!(
            stdout.contains(branch),
            "Branch {} should be in completion output",
            branch
        );
    }
}

#[test]
fn test_complete_stops_after_branch_provided() {
    let temp = TestRepo::new();
    temp.commit("initial");

    // Create branches
    StdCommand::new("git")
        .args(["branch", "feature/one"])
        .current_dir(temp.root_path())
        .output()
        .unwrap();

    StdCommand::new("git")
        .args(["branch", "feature/two"])
        .current_dir(temp.root_path())
        .output()
        .unwrap();

    // Test that switch stops completing after branch is provided
    let mut cmd = Command::cargo_bin("wt").unwrap();
    let output = cmd
        .current_dir(temp.root_path())
        .args(["complete", "wt", "switch", "feature/one", ""])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "");

    // Test that push stops completing after branch is provided
    let mut cmd = Command::cargo_bin("wt").unwrap();
    let output = cmd
        .current_dir(temp.root_path())
        .args(["complete", "wt", "push", "feature/one", ""])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "");

    // Test that merge stops completing after branch is provided
    let mut cmd = Command::cargo_bin("wt").unwrap();
    let output = cmd
        .current_dir(temp.root_path())
        .args(["complete", "wt", "merge", "feature/one", ""])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "");
}

#[test]
fn test_complete_switch_with_create_flag_no_completion() {
    let temp = TestRepo::new();
    temp.commit("initial");

    StdCommand::new("git")
        .args(["branch", "feature/existing"])
        .current_dir(temp.root_path())
        .output()
        .unwrap();

    // Test with --create flag (long form)
    let mut cmd = Command::cargo_bin("wt").unwrap();
    let output = cmd
        .current_dir(temp.root_path())
        .args(["complete", "wt", "switch", "--create", ""])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "");

    // Test with -c flag (short form)
    let mut cmd = Command::cargo_bin("wt").unwrap();
    let output = cmd
        .current_dir(temp.root_path())
        .args(["complete", "wt", "switch", "-c", ""])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "");
}

#[test]
fn test_complete_switch_base_flag_after_branch() {
    let temp = TestRepo::new();
    temp.commit("initial");

    // Create branches
    StdCommand::new("git")
        .args(["branch", "develop"])
        .current_dir(temp.root_path())
        .output()
        .unwrap();

    // Test completion for --base even after --create and branch name
    let mut cmd = Command::cargo_bin("wt").unwrap();
    let output = cmd
        .current_dir(temp.root_path())
        .args([
            "complete",
            "wt",
            "switch",
            "--create",
            "new-feature",
            "--base",
            "",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should complete base flag value with branches
    assert!(stdout.contains("develop"));
}

#[test]
fn test_complete_remove_shows_branches() {
    let mut temp = TestRepo::new();
    temp.commit("initial");

    // Create worktree (creates "feature/new" branch)
    temp.add_worktree("feature-worktree", "feature/new");

    // Create another branch without worktree
    StdCommand::new("git")
        .args(["branch", "hotfix/bug"])
        .current_dir(temp.root_path())
        .output()
        .unwrap();

    // Test completion for remove (should show ALL branches)
    let mut cmd = Command::cargo_bin("wt").unwrap();
    let output = cmd
        .current_dir(temp.root_path())
        .args(["complete", "wt", "remove", ""])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    let branches: Vec<&str> = stdout.lines().collect();

    // Should include both branches
    assert!(branches.iter().any(|b| b.contains("feature/new")));
    assert!(branches.iter().any(|b| b.contains("hotfix/bug")));
}
