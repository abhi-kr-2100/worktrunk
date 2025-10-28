use worktrunk::config::{ProjectConfig, WorktrunkConfig};
use worktrunk::git::{GitError, GitResultExt, Repository};
use worktrunk::styling::{AnstyleStyle, CYAN, CYAN_BOLD, format_with_gutter};

use super::command_executor::{CommandContext, prepare_project_commands};
use super::worktree::handle_push;
use super::worktree::handle_remove;
use crate::output::execute_command_in_worktree;

pub fn handle_merge(
    target: Option<&str>,
    squash: bool,
    keep: bool,
    message: Option<&str>,
    no_hooks: bool,
    force: bool,
) -> Result<(), GitError> {
    let repo = Repository::current();

    // Show progress for initial validation
    crate::output::progress(format!("🔄 {CYAN}Validating merge...{CYAN:#}"))?;

    // Get current branch
    let current_branch = repo.current_branch()?.ok_or(GitError::DetachedHead)?;

    // Get target branch (default to default branch if not provided)
    let target_branch = target.map_or_else(|| repo.default_branch(), |b| Ok(b.to_string()))?;

    // Check if already on target branch
    if current_branch == target_branch {
        use worktrunk::styling::{GREEN, SUCCESS_EMOJI};
        let green_bold = GREEN.bold();
        crate::output::success(format!(
            "{SUCCESS_EMOJI} {GREEN}Already on {green_bold}{target_branch}{green_bold:#}, nothing to merge{GREEN:#}"
        ))?;
        return Ok(());
    }

    // Load config for LLM integration
    let config = WorktrunkConfig::load().git_context("Failed to load config")?;

    // Run pre-merge checks unless --no-hooks was specified
    // Do this BEFORE committing so we fail fast if checks won't pass
    if !no_hooks && let Ok(Some(project_config)) = ProjectConfig::load(&repo.worktree_root()?) {
        let worktree_path =
            std::env::current_dir().git_context("Failed to get current directory")?;
        run_pre_merge_commands(
            &project_config,
            &current_branch,
            &target_branch,
            &worktree_path,
            &repo,
            &config,
            force,
        )?;
    }

    // Auto-commit uncommitted changes if they exist
    // Only do this after pre-merge checks pass
    if repo.is_dirty()? {
        handle_commit_changes(message, &config.commit_generation)?;
    }

    // Squash commits if requested
    if squash {
        handle_squash(&target_branch)?;
    }

    // Rebase onto target
    crate::output::progress(format!(
        "🔄 {CYAN}Rebasing onto {CYAN_BOLD}{target_branch}{CYAN_BOLD:#}...{CYAN:#}"
    ))?;

    let rebase_result = repo.run_command(&["rebase", &target_branch]);

    // If rebase failed, check if it's due to conflicts
    if let Err(e) = rebase_result {
        if let Some(state) = repo.worktree_state()?
            && state.starts_with("REBASING")
        {
            return Err(GitError::RebaseConflict {
                state,
                target_branch: target_branch.to_string(),
            });
        }
        // Not a rebase conflict, return original error
        return Err(GitError::CommandFailed(format!(
            "Failed to rebase onto '{}': {}",
            target_branch, e
        )));
    }

    // Verify rebase completed successfully (safety check for edge cases)
    if let Some(state) = repo.worktree_state()? {
        return Err(GitError::RebaseConflict {
            state,
            target_branch: target_branch.to_string(),
        });
    }

    // Fast-forward push to target branch (reuse handle_push logic)
    handle_push(Some(&target_branch), false)?;

    // Execute post-merge commands in the main worktree
    let main_worktree_path = repo.main_worktree_root()?;
    execute_post_merge_commands(
        &main_worktree_path,
        &repo,
        &config,
        &current_branch,
        &target_branch,
        force,
    )?;

    // Finish worktree unless --keep was specified
    if !keep {
        crate::output::progress(format!("🔄 {CYAN}Cleaning up worktree...{CYAN:#}"))?;

        // Get primary worktree path before finishing (while we can still run git commands)
        let primary_worktree_dir = repo.main_worktree_root()?;

        let result = handle_remove(None)?;

        // Set directory for shell integration (but don't print separate success message)
        if let super::worktree::RemoveResult::RemovedWorktree { primary_path } = &result {
            crate::output::change_directory(primary_path)?;
        }

        // Check if we need to switch to target branch
        let primary_repo = Repository::at(&primary_worktree_dir);
        let new_branch = primary_repo.current_branch()?;
        if new_branch.as_deref() != Some(&target_branch) {
            crate::output::progress(format!(
                "🔄 {CYAN}Switching to {CYAN_BOLD}{target_branch}{CYAN_BOLD:#}...{CYAN:#}"
            ))?;
            primary_repo
                .run_command(&["switch", &target_branch])
                .git_context(&format!("Failed to switch to '{}'", target_branch))?;
        }

        // Print comprehensive summary
        crate::output::progress("")?;
        handle_merge_summary_output(Some(&primary_worktree_dir))?;
    } else {
        // Print comprehensive summary (worktree preserved)
        crate::output::progress("")?;
        handle_merge_summary_output(None)?;
    }

    Ok(())
}

/// Format the merge summary message (includes emoji and color for consistency)
fn format_merge_summary(primary_path: Option<&std::path::Path>) -> String {
    use worktrunk::styling::{GREEN, SUCCESS_EMOJI};

    // Show where we ended up
    if let Some(path) = primary_path {
        format!(
            "{SUCCESS_EMOJI} {GREEN}Returned to primary at {}{GREEN:#}",
            path.display()
        )
    } else {
        format!("{SUCCESS_EMOJI} {GREEN}Kept worktree (use 'wt remove' to clean up){GREEN:#}")
    }
}

/// Handle output for merge summary using global output context
fn handle_merge_summary_output(primary_path: Option<&std::path::Path>) -> Result<(), GitError> {
    let message = format_merge_summary(primary_path);

    // Show success message (formatting added by OutputContext)
    crate::output::success(message)?;

    // Flush output
    crate::output::flush()?;

    Ok(())
}

/// Format a commit message with the first line in bold, ready for gutter display
fn format_commit_message_for_display(message: &str) -> String {
    let bold = AnstyleStyle::new().bold();
    let lines: Vec<&str> = message.lines().collect();

    if lines.is_empty() {
        return String::new();
    }

    // Format first line in bold
    let mut result = format!("{bold}{}{bold:#}", lines[0]);

    // Add remaining lines without bold
    if lines.len() > 1 {
        for line in &lines[1..] {
            result.push('\n');
            result.push_str(line);
        }
    }

    result
}

/// Commit uncommitted changes with LLM-generated message
fn handle_commit_changes(
    custom_instruction: Option<&str>,
    commit_generation_config: &worktrunk::config::CommitGenerationConfig,
) -> Result<(), GitError> {
    let repo = Repository::current();

    crate::output::progress(format!(
        "🔄 {CYAN}Committing uncommitted changes...{CYAN:#}"
    ))?;

    // Stage all changes including untracked files
    repo.run_command(&["add", "-A"])
        .git_context("Failed to stage changes")?;

    // Generate commit message
    crate::output::progress(format!("🔄 {CYAN}Generating commit message...{CYAN:#}"))?;

    let commit_message =
        crate::llm::generate_commit_message(custom_instruction, commit_generation_config)?;

    // Display the generated commit message
    let formatted_message = format_commit_message_for_display(&commit_message);
    crate::output::progress(format_with_gutter(&formatted_message, "", None))?;

    // Commit
    repo.run_command(&["commit", "-m", &commit_message])
        .git_context("Failed to commit")?;

    use worktrunk::styling::{GREEN, SUCCESS_EMOJI};
    crate::output::success(format!("{SUCCESS_EMOJI} {GREEN}Committed changes{GREEN:#}"))?;

    Ok(())
}

fn handle_squash(target_branch: &str) -> Result<Option<usize>, GitError> {
    let repo = Repository::current();

    // Get merge base with target branch
    let merge_base = repo.merge_base("HEAD", target_branch)?;

    // Count commits since merge base
    let commit_count = repo.count_commits(&merge_base, "HEAD")?;

    // Check if there are staged changes
    let has_staged = repo.has_staged_changes()?;

    // Handle different scenarios
    if commit_count == 0 && !has_staged {
        // No commits and no staged changes - nothing to squash
        let dim = AnstyleStyle::new().dimmed();
        crate::output::progress(format!(
            "{dim}No commits to squash - already at merge base{dim:#}"
        ))?;
        return Ok(None);
    }

    if commit_count == 0 && has_staged {
        // Just staged changes, no commits - would need to commit but this shouldn't happen in merge flow
        return Err(GitError::StagedChangesWithoutCommits);
    }

    if commit_count == 1 && !has_staged {
        // Single commit, no staged changes - nothing to do
        let dim = AnstyleStyle::new().dimmed();
        crate::output::progress(format!(
            "{dim}Only 1 commit since {CYAN_BOLD}{target_branch}{CYAN_BOLD:#} - no squashing needed{dim:#}"
        ))?;
        return Ok(None);
    }

    // One or more commits (possibly with staged changes) - squash them
    crate::output::progress(format!(
        "🔄 {CYAN}Squashing {commit_count} commits into one...{CYAN:#}"
    ))?;

    // Get commit subjects for the squash message
    let range = format!("{}..HEAD", merge_base);
    let subjects = repo.commit_subjects(&range)?;

    // Load config and generate commit message
    crate::output::progress(format!(
        "🔄 {CYAN}Generating squash commit message...{CYAN:#}"
    ))?;

    let config = WorktrunkConfig::load().git_context("Failed to load config")?;
    let commit_message =
        crate::llm::generate_squash_message(target_branch, &subjects, &config.commit_generation)
            .git_context("Failed to generate commit message")?;

    // Display the generated commit message
    let formatted_message = format_commit_message_for_display(&commit_message);
    crate::output::progress(format_with_gutter(&formatted_message, "", None))?;

    // Reset to merge base (soft reset stages all changes)
    repo.run_command(&["reset", "--soft", &merge_base])
        .git_context("Failed to reset to merge base")?;

    // Commit with the generated message
    repo.run_command(&["commit", "-m", &commit_message])
        .git_context("Failed to create squash commit")?;

    // Show success immediately after completing the squash
    use worktrunk::styling::{GREEN, SUCCESS_EMOJI};
    crate::output::success(format!(
        "{SUCCESS_EMOJI} {GREEN}Squashed {commit_count} commits into one{GREEN:#}"
    ))?;

    Ok(Some(commit_count))
}

/// Run pre-merge commands sequentially (blocking, fail-fast)
fn run_pre_merge_commands(
    project_config: &ProjectConfig,
    current_branch: &str,
    target_branch: &str,
    worktree_path: &std::path::Path,
    repo: &Repository,
    config: &WorktrunkConfig,
    force: bool,
) -> Result<(), GitError> {
    let Some(pre_merge_config) = &project_config.pre_merge_command else {
        return Ok(());
    };

    let ctx = CommandContext::new(repo, config, current_branch, worktree_path, force);
    let commands = prepare_project_commands(
        pre_merge_config,
        "cmd",
        &ctx,
        false,
        &[("target", target_branch)],
        "Pre-merge commands",
        |_, command| {
            let dim = AnstyleStyle::new().dimmed();
            crate::output::progress(format!("{dim}Skipping pre-merge command: {command}{dim:#}"))
                .ok();
        },
    )?;
    for prepared in commands {
        crate::output::progress(format!(
            "🔄 {CYAN}Running pre-merge command {CYAN_BOLD}{name}{CYAN_BOLD:#}:{CYAN:#}",
            name = prepared.name
        ))?;
        crate::output::progress(format_with_gutter(&prepared.expanded, "", None))?;

        if let Err(e) = execute_command_in_worktree(worktree_path, &prepared.expanded) {
            return Err(GitError::PreMergeCommandFailed {
                command_name: prepared.name.clone(),
                error: e.to_string(),
            });
        }

        // No need to flush here - the redirect in execute_command_in_worktree ensures ordering
    }

    Ok(())
}

/// Load project configuration with proper error conversion
fn load_project_config(repo: &Repository) -> Result<Option<ProjectConfig>, GitError> {
    let repo_root = repo.worktree_root()?;
    ProjectConfig::load(&repo_root).git_context("Failed to load project config")
}

/// Execute post-merge commands sequentially in the main worktree (blocking)
fn execute_post_merge_commands(
    main_worktree_path: &std::path::Path,
    repo: &Repository,
    config: &WorktrunkConfig,
    branch: &str,
    target_branch: &str,
    force: bool,
) -> Result<(), GitError> {
    use worktrunk::styling::WARNING;

    let project_config = match load_project_config(repo)? {
        Some(cfg) => cfg,
        None => return Ok(()),
    };

    let Some(post_merge_config) = &project_config.post_merge_command else {
        return Ok(());
    };

    let ctx = CommandContext::new(repo, config, branch, main_worktree_path, force);
    let commands = prepare_project_commands(
        post_merge_config,
        "cmd",
        &ctx,
        false,
        &[("target", target_branch)],
        "Post-merge commands",
        |_, command| {
            let dim = AnstyleStyle::new().dimmed();
            crate::output::progress(format!("{dim}Skipping command: {command}{dim:#}")).ok();
        },
    )?;

    if commands.is_empty() {
        return Ok(());
    }

    // Execute each command sequentially in the main worktree
    for prepared in commands {
        crate::output::progress(format!(
            "🔄 {CYAN}Running post-merge command {CYAN_BOLD}{name}{CYAN_BOLD:#}:{CYAN:#}",
            name = prepared.name
        ))?;
        crate::output::progress(format_with_gutter(&prepared.expanded, "", None))?;

        if let Err(e) = execute_command_in_worktree(main_worktree_path, &prepared.expanded) {
            use worktrunk::styling::WARNING_EMOJI;
            let warning_bold = WARNING.bold();
            crate::output::progress(format!(
                "{WARNING_EMOJI} {WARNING}Command {warning_bold}{name}{warning_bold:#} failed: {e}{WARNING:#}",
                name = prepared.name,
            ))?;
            // Continue with other commands even if one fails
        }
    }

    crate::output::flush()?;

    Ok(())
}
