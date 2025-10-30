use worktrunk::config::{ProjectConfig, WorktrunkConfig};
use worktrunk::git::{GitError, GitResultExt, Repository};
use worktrunk::styling::{AnstyleStyle, HINT, HINT_EMOJI};

use super::merge::{execute_post_merge_commands, run_pre_merge_commands};
use super::worktree::{execute_post_create_commands, execute_post_start_commands_sequential};

/// Handle `wt dev run-hook` command
pub fn handle_dev_run_hook(hook_type: &str, force: bool) -> Result<(), GitError> {
    // Derive context from current environment
    let repo = Repository::current();
    let worktree_path = std::env::current_dir()
        .map_err(|e| GitError::CommandFailed(format!("Failed to get current directory: {}", e)))?;
    let branch = repo
        .current_branch()
        .git_context("Failed to get current branch")?
        .ok_or_else(|| GitError::CommandFailed("Not on a branch (detached HEAD)".to_string()))?;
    let config = WorktrunkConfig::load().git_context("Failed to load config")?;

    // Load project config (show helpful error if missing)
    let project_config = load_project_config(&repo)?;

    // TODO: Add support for custom variable overrides (e.g., --var key=value)
    // This would allow testing hooks with different contexts without being in that context

    // Execute the hook based on type
    match hook_type {
        "post-create" => {
            check_hook_configured(&project_config.post_create_command, "post-create")?;
            execute_post_create_commands(&worktree_path, &repo, &config, &branch, force)
        }
        "post-start" => {
            check_hook_configured(&project_config.post_start_command, "post-start")?;
            execute_post_start_commands_sequential(&worktree_path, &repo, &config, &branch, force)
        }
        "pre-merge" => {
            check_hook_configured(&project_config.pre_merge_command, "pre-merge")?;
            let target_branch = repo.default_branch().unwrap_or_else(|_| "main".to_string());
            run_pre_merge_commands(
                &project_config,
                &branch,
                &target_branch,
                &worktree_path,
                &repo,
                &config,
                force,
            )
        }
        "post-merge" => {
            check_hook_configured(&project_config.post_merge_command, "post-merge")?;
            let target_branch = repo.default_branch().unwrap_or_else(|_| "main".to_string());
            execute_post_merge_commands(
                &worktree_path,
                &repo,
                &config,
                &branch,
                &target_branch,
                force,
            )
        }
        _ => Err(GitError::CommandFailed(format!(
            "Unknown hook type: {}",
            hook_type
        ))),
    }
}

fn load_project_config(repo: &Repository) -> Result<ProjectConfig, GitError> {
    let repo_root = repo.worktree_root()?;
    let config_path = repo_root.join(".config").join("wt.toml");

    match ProjectConfig::load(&repo_root).git_context("Failed to load project config")? {
        Some(cfg) => Ok(cfg),
        None => {
            // No project config found - show helpful error
            let bold = AnstyleStyle::new().bold();
            use worktrunk::styling::ERROR;
            use worktrunk::styling::ERROR_EMOJI;
            eprintln!("{ERROR_EMOJI} {ERROR}No project configuration found{ERROR:#}",);
            eprintln!(
                "{HINT_EMOJI} {HINT}Create a config file at: {bold}{}{bold:#}{HINT:#}",
                config_path.display()
            );
            Err(GitError::CommandFailed(
                "No project configuration found".to_string(),
            ))
        }
    }
}

fn check_hook_configured<T>(hook: &Option<T>, hook_name: &str) -> Result<(), GitError> {
    if hook.is_none() {
        eprintln!(
            "{HINT_EMOJI} {HINT}No {hook_name} commands configured in project config{HINT:#}"
        );
        return Err(GitError::CommandFailed(format!(
            "No {hook_name} commands configured"
        )));
    }
    Ok(())
}
