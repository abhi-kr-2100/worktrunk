use crate::common::TestRepo;
use insta::Settings;
use insta_cmd::{assert_cmd_snapshot, get_cargo_bin};
use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Test configure-shell with dry-run flag
#[test]
fn test_configure_shell_dry_run() {
    let repo = TestRepo::new();
    let temp_home = TempDir::new().unwrap();

    // Create a fake .zshrc file
    let zshrc_path = temp_home.path().join(".zshrc");
    fs::write(&zshrc_path, "# Existing config\n").unwrap();

    let mut settings = Settings::clone_current();
    settings.set_snapshot_path("../snapshots");
    settings.add_filter(&temp_home.path().to_string_lossy(), "[TEMP_HOME]");

    settings.bind(|| {
        let mut cmd = Command::new(get_cargo_bin("wt"));
        repo.clean_cli_env(&mut cmd);
        cmd.arg("configure-shell")
            .arg("--dry-run")
            .env("HOME", temp_home.path())
            .current_dir(repo.root_path());

        assert_cmd_snapshot!("configure_shell_dry_run", cmd);
    });
}

/// Test configure-shell with specific shell
#[test]
fn test_configure_shell_specific_shell() {
    let repo = TestRepo::new();
    let temp_home = TempDir::new().unwrap();

    // Create a fake .zshrc file
    let zshrc_path = temp_home.path().join(".zshrc");
    fs::write(&zshrc_path, "# Existing config\n").unwrap();

    let mut settings = Settings::clone_current();
    settings.set_snapshot_path("../snapshots");
    settings.add_filter(&temp_home.path().to_string_lossy(), "[TEMP_HOME]");

    settings.bind(|| {
        let mut cmd = Command::new(get_cargo_bin("wt"));
        repo.clean_cli_env(&mut cmd);
        cmd.arg("configure-shell")
            .arg("--shell")
            .arg("zsh")
            .env("HOME", temp_home.path())
            .current_dir(repo.root_path());

        assert_cmd_snapshot!("configure_shell_zsh", cmd);
    });

    // Verify the file was modified
    let content = fs::read_to_string(&zshrc_path).unwrap();
    assert!(content.contains("eval \"$(wt init zsh)\""));
}

/// Test configure-shell when line already exists
#[test]
fn test_configure_shell_already_exists() {
    let repo = TestRepo::new();
    let temp_home = TempDir::new().unwrap();

    // Create a fake .zshrc file with the line already present
    let zshrc_path = temp_home.path().join(".zshrc");
    fs::write(&zshrc_path, "# Existing config\neval \"$(wt init zsh)\"\n").unwrap();

    let mut settings = Settings::clone_current();
    settings.set_snapshot_path("../snapshots");
    settings.add_filter(&temp_home.path().to_string_lossy(), "[TEMP_HOME]");

    settings.bind(|| {
        let mut cmd = Command::new(get_cargo_bin("wt"));
        repo.clean_cli_env(&mut cmd);
        cmd.arg("configure-shell")
            .arg("--shell")
            .arg("zsh")
            .env("HOME", temp_home.path())
            .current_dir(repo.root_path());

        assert_cmd_snapshot!("configure_shell_already_exists", cmd);
    });

    // Verify the file was not modified (no duplicate)
    let content = fs::read_to_string(&zshrc_path).unwrap();
    let count = content.matches("wt init").count();
    assert_eq!(count, 1, "Should only have one wt init line");
}

/// Test configure-shell with custom command prefix
#[test]
fn test_configure_shell_custom_prefix() {
    let repo = TestRepo::new();
    let temp_home = TempDir::new().unwrap();

    // Create the appropriate bash config file based on platform
    let bash_config_path = if cfg!(target_os = "macos") {
        temp_home.path().join(".bash_profile")
    } else {
        temp_home.path().join(".bashrc")
    };
    fs::write(&bash_config_path, "# Existing config\n").unwrap();

    let mut settings = Settings::clone_current();
    settings.set_snapshot_path("../snapshots");
    settings.add_filter(&temp_home.path().to_string_lossy(), "[TEMP_HOME]");

    settings.bind(|| {
        let mut cmd = Command::new(get_cargo_bin("wt"));
        repo.clean_cli_env(&mut cmd);
        cmd.arg("configure-shell")
            .arg("--shell")
            .arg("bash")
            .arg("--cmd")
            .arg("worktree")
            .env("HOME", temp_home.path())
            .current_dir(repo.root_path());

        assert_cmd_snapshot!("configure_shell_custom_prefix", cmd);
    });

    // Verify the file has the custom prefix
    let content = fs::read_to_string(&bash_config_path).unwrap();
    assert!(content.contains("eval \"$(worktree init bash)\""));
}

/// Test configure-shell for Fish (creates new file in conf.d/)
#[test]
fn test_configure_shell_fish() {
    let repo = TestRepo::new();
    let temp_home = TempDir::new().unwrap();

    let mut settings = Settings::clone_current();
    settings.set_snapshot_path("../snapshots");
    settings.add_filter(&temp_home.path().to_string_lossy(), "[TEMP_HOME]");

    settings.bind(|| {
        let mut cmd = Command::new(get_cargo_bin("wt"));
        repo.clean_cli_env(&mut cmd);
        cmd.arg("configure-shell")
            .arg("--shell")
            .arg("fish")
            .env("HOME", temp_home.path())
            .current_dir(repo.root_path());

        assert_cmd_snapshot!("configure_shell_fish", cmd);
    });

    // Verify the fish conf.d file was created
    let fish_config = temp_home.path().join(".config/fish/conf.d/wt.fish");
    assert!(fish_config.exists(), "Fish config file should be created");

    let content = fs::read_to_string(&fish_config).unwrap();
    assert!(
        content.contains("wt init fish | source"),
        "Should contain wt init command"
    );
}

/// Test configure-shell when no config files exist
#[test]
fn test_configure_shell_no_files() {
    let repo = TestRepo::new();
    let temp_home = TempDir::new().unwrap();

    let mut settings = Settings::clone_current();
    settings.set_snapshot_path("../snapshots");
    settings.add_filter(&temp_home.path().to_string_lossy(), "[TEMP_HOME]");

    settings.bind(|| {
        let mut cmd = Command::new(get_cargo_bin("wt"));
        repo.clean_cli_env(&mut cmd);
        cmd.arg("configure-shell")
            .env("HOME", temp_home.path())
            .current_dir(repo.root_path());

        assert_cmd_snapshot!("configure_shell_no_files", cmd);
    });
}

/// Test configure-shell for Fish with custom prefix
#[test]
fn test_configure_shell_fish_custom_prefix() {
    let repo = TestRepo::new();
    let temp_home = TempDir::new().unwrap();

    let mut settings = Settings::clone_current();
    settings.set_snapshot_path("../snapshots");
    settings.add_filter(&temp_home.path().to_string_lossy(), "[TEMP_HOME]");

    settings.bind(|| {
        let mut cmd = Command::new(get_cargo_bin("wt"));
        repo.clean_cli_env(&mut cmd);
        cmd.arg("configure-shell")
            .arg("--shell")
            .arg("fish")
            .arg("--cmd")
            .arg("worktree")
            .env("HOME", temp_home.path())
            .current_dir(repo.root_path());

        assert_cmd_snapshot!("configure_shell_fish_custom_prefix", cmd);
    });

    // Verify the fish conf.d file was created with correct prefix in filename
    let fish_config = temp_home.path().join(".config/fish/conf.d/worktree.fish");
    assert!(
        fish_config.exists(),
        "Fish config file should be created with custom prefix in filename"
    );

    let content = fs::read_to_string(&fish_config).unwrap();
    assert!(
        content.contains("worktree init fish | source"),
        "Should contain worktree init command with custom prefix"
    );
}

/// Test configure-shell with multiple existing config files
#[test]
fn test_configure_shell_multiple_configs() {
    let repo = TestRepo::new();
    let temp_home = TempDir::new().unwrap();

    // Create multiple shell config files (platform-aware for Bash)
    let bash_config_path = if cfg!(target_os = "macos") {
        temp_home.path().join(".bash_profile")
    } else {
        temp_home.path().join(".bashrc")
    };
    let zshrc_path = temp_home.path().join(".zshrc");
    fs::write(&bash_config_path, "# Existing bash config\n").unwrap();
    fs::write(&zshrc_path, "# Existing zsh config\n").unwrap();

    let mut settings = Settings::clone_current();
    settings.set_snapshot_path("../snapshots");
    settings.add_filter(&temp_home.path().to_string_lossy(), "[TEMP_HOME]");

    settings.bind(|| {
        let mut cmd = Command::new(get_cargo_bin("wt"));
        repo.clean_cli_env(&mut cmd);
        cmd.arg("configure-shell")
            .env("HOME", temp_home.path())
            .current_dir(repo.root_path());

        assert_cmd_snapshot!("configure_shell_multiple_configs", cmd);
    });

    // Verify both files were modified
    let bash_content = fs::read_to_string(&bash_config_path).unwrap();
    assert!(
        bash_content.contains("eval \"$(wt init bash)\""),
        "Bash config should be updated"
    );

    let zsh_content = fs::read_to_string(&zshrc_path).unwrap();
    assert!(
        zsh_content.contains("eval \"$(wt init zsh)\""),
        "Zsh config should be updated"
    );
}

/// Test configure-shell shows both shells needing updates and already configured shells
#[test]
fn test_configure_shell_mixed_states() {
    let repo = TestRepo::new();
    let temp_home = TempDir::new().unwrap();

    // Create bash config with wt already configured
    let bash_config_path = if cfg!(target_os = "macos") {
        temp_home.path().join(".bash_profile")
    } else {
        temp_home.path().join(".bashrc")
    };
    fs::write(
        &bash_config_path,
        "# Existing config\neval \"$(wt init bash)\"\n",
    )
    .unwrap();

    // Create zsh config without wt
    let zshrc_path = temp_home.path().join(".zshrc");
    fs::write(&zshrc_path, "# Existing zsh config\n").unwrap();

    let mut settings = Settings::clone_current();
    settings.set_snapshot_path("../snapshots");
    settings.add_filter(&temp_home.path().to_string_lossy(), "[TEMP_HOME]");

    settings.bind(|| {
        let mut cmd = Command::new(get_cargo_bin("wt"));
        repo.clean_cli_env(&mut cmd);
        cmd.arg("configure-shell")
            .env("HOME", temp_home.path())
            .current_dir(repo.root_path());

        assert_cmd_snapshot!("configure_shell_mixed_states", cmd);
    });

    // Verify bash was not modified (already configured)
    let bash_content = fs::read_to_string(&bash_config_path).unwrap();
    let bash_wt_count = bash_content.matches("wt init").count();
    assert_eq!(
        bash_wt_count, 1,
        "Bash should still have exactly one wt init line"
    );

    // Verify zsh was modified
    let zsh_content = fs::read_to_string(&zshrc_path).unwrap();
    assert!(
        zsh_content.contains("eval \"$(wt init zsh)\""),
        "Zsh config should be updated"
    );
}

/// Test configure-shell --dry-run shows both shells needing updates and already configured shells
#[test]
fn test_configure_shell_mixed_states_dry_run() {
    let repo = TestRepo::new();
    let temp_home = TempDir::new().unwrap();

    // Create bash config with wt already configured
    let bash_config_path = if cfg!(target_os = "macos") {
        temp_home.path().join(".bash_profile")
    } else {
        temp_home.path().join(".bashrc")
    };
    fs::write(
        &bash_config_path,
        "# Existing config\neval \"$(wt init bash)\"\n",
    )
    .unwrap();

    // Create zsh config without wt
    let zshrc_path = temp_home.path().join(".zshrc");
    fs::write(&zshrc_path, "# Existing zsh config\n").unwrap();

    let mut settings = Settings::clone_current();
    settings.set_snapshot_path("../snapshots");
    settings.add_filter(&temp_home.path().to_string_lossy(), "[TEMP_HOME]");

    settings.bind(|| {
        let mut cmd = Command::new(get_cargo_bin("wt"));
        repo.clean_cli_env(&mut cmd);
        cmd.arg("configure-shell")
            .arg("--dry-run")
            .env("HOME", temp_home.path())
            .current_dir(repo.root_path());

        assert_cmd_snapshot!("configure_shell_mixed_states_dry_run", cmd);
    });

    // Verify files were not modified in dry-run
    let bash_content = fs::read_to_string(&bash_config_path).unwrap();
    let bash_wt_count = bash_content.matches("wt init").count();
    assert_eq!(
        bash_wt_count, 1,
        "Bash should still have exactly one wt init line"
    );

    let zsh_content = fs::read_to_string(&zshrc_path).unwrap();
    assert!(
        !zsh_content.contains("eval \"$(wt init zsh)\""),
        "Zsh config should not be modified in dry-run"
    );
}

/// Test configure-shell detects Fish when conf.d directory exists
#[test]
fn test_configure_shell_fish_conf_d_exists() {
    let repo = TestRepo::new();
    let temp_home = TempDir::new().unwrap();

    // Create Fish conf.d directory (but not the wt.fish file)
    let fish_conf_d = temp_home.path().join(".config/fish/conf.d");
    fs::create_dir_all(&fish_conf_d).unwrap();

    let mut settings = Settings::clone_current();
    settings.set_snapshot_path("../snapshots");
    settings.add_filter(&temp_home.path().to_string_lossy(), "[TEMP_HOME]");

    settings.bind(|| {
        let mut cmd = Command::new(get_cargo_bin("wt"));
        repo.clean_cli_env(&mut cmd);
        cmd.arg("configure-shell")
            .arg("--dry-run")
            .env("HOME", temp_home.path())
            .current_dir(repo.root_path());

        assert_cmd_snapshot!("configure_shell_fish_conf_d_exists", cmd);
    });

    // Verify the fish file was not created in dry-run
    let fish_config = temp_home.path().join(".config/fish/conf.d/wt.fish");
    assert!(
        !fish_config.exists(),
        "Fish config file should not be created in dry-run"
    );
}
