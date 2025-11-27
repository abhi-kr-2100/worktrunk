//! Worktrunk error types and formatting
//!
//! This module provides typed error handling:
//!
//! - **`GitError`** - A typed enum for domain errors that can be pattern-matched
//!   and tested. Use `.into()` to convert to `anyhow::Error` while preserving the
//!   type for pattern matching. Display produces styled output for users.
//!
//! - **`WorktrunkError`** - A minimal enum for semantic errors that need
//!   special handling (exit codes, silent errors).

use std::path::PathBuf;

use super::HookType;
use crate::path::format_path_for_display;
use crate::styling::{
    ERROR, ERROR_BOLD, ERROR_EMOJI, HINT, HINT_BOLD, HINT_EMOJI, INFO_EMOJI, format_with_gutter,
};

// =============================================================================
// GitError - Typed domain errors
// =============================================================================

/// Domain errors for git and worktree operations.
///
/// This enum provides structured error data that can be pattern-matched and tested.
/// Each variant stores the data needed to construct a user-facing error message.
/// Display produces styled output with emoji and colors.
///
/// # Usage
///
/// ```ignore
/// // Return a typed error (Display produces styled output)
/// return Err(GitError::DetachedHead { action: Some("merge".into()) }.into());
///
/// // Pattern match on errors
/// if let Some(GitError::BranchAlreadyExists { branch }) = err.downcast_ref() {
///     println!("Branch {} exists", branch);
/// }
/// ```
#[derive(Debug, Clone)]
pub enum GitError {
    // -------------------------------------------------------------------------
    // Git state errors
    // -------------------------------------------------------------------------
    /// HEAD is detached (not on a branch)
    DetachedHead {
        /// Action that was blocked (e.g., "merge", "push")
        action: Option<String>,
    },

    /// Working tree has uncommitted changes
    UncommittedChanges {
        /// Action that was blocked (e.g., "remove worktree")
        action: Option<String>,
    },

    /// Branch already exists
    BranchAlreadyExists { branch: String },

    // -------------------------------------------------------------------------
    // Worktree errors
    // -------------------------------------------------------------------------
    /// Worktree directory is missing from filesystem
    WorktreeMissing { branch: String },

    /// No worktree exists for the given branch
    NoWorktreeFound { branch: String },

    /// Target path for worktree is already occupied
    WorktreePathOccupied {
        branch: String,
        path: PathBuf,
        /// Branch currently using this path, if known
        occupant: Option<String>,
    },

    /// Directory already exists at target path
    WorktreePathExists { path: PathBuf },

    /// Failed to create worktree
    WorktreeCreationFailed {
        branch: String,
        base_branch: Option<String>,
        error: String,
    },

    /// Failed to remove worktree
    WorktreeRemovalFailed {
        branch: String,
        path: PathBuf,
        error: String,
    },

    // -------------------------------------------------------------------------
    // Merge/push errors
    // -------------------------------------------------------------------------
    /// Conflicting uncommitted changes prevent operation
    ConflictingChanges {
        files: Vec<String>,
        worktree_path: PathBuf,
    },

    /// Push is not fast-forward
    NotFastForward {
        target_branch: String,
        /// Formatted commit list for display
        commits_formatted: String,
        /// Whether this error occurred during a merge operation (affects hint message)
        in_merge_context: bool,
    },

    /// Merge commits found in push range
    MergeCommitsFound,

    /// Rebase stopped due to conflicts
    RebaseConflict {
        target_branch: String,
        git_output: String,
    },

    /// Push operation failed
    PushFailed { error: String },

    // -------------------------------------------------------------------------
    // Validation errors
    // -------------------------------------------------------------------------
    /// Cannot prompt in non-interactive environment
    NotInteractive,

    /// Parse error (for git output parsing failures)
    ParseError { message: String },

    /// LLM command failed during commit generation
    LlmCommandFailed {
        /// The full command string (e.g., "llm --model claude")
        command: String,
        /// The stderr output from the command
        error: String,
    },

    /// Project configuration file not found
    ProjectConfigNotFound {
        /// Path where config file was expected
        config_path: PathBuf,
    },

    /// Generic error with custom message
    Other { message: String },
}

impl std::error::Error for GitError {}

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GitError::DetachedHead { action } => {
                let message = match action {
                    Some(action) => format!("Cannot {action}: not on a branch (detached HEAD)"),
                    None => "Not on a branch (detached HEAD)".to_string(),
                };
                write!(
                    f,
                    "{ERROR_EMOJI} {ERROR}{message}{ERROR:#}\n\n{HINT_EMOJI} {HINT}Switch to a branch first with 'git switch <branch>'{HINT:#}"
                )
            }

            GitError::UncommittedChanges { action } => {
                let message = match action {
                    Some(action) => {
                        format!("Cannot {action}: working tree has uncommitted changes")
                    }
                    None => "Working tree has uncommitted changes".to_string(),
                };
                write!(
                    f,
                    "{ERROR_EMOJI} {ERROR}{message}{ERROR:#}\n\n{HINT_EMOJI} {HINT}Commit or stash them first{HINT:#}"
                )
            }

            GitError::BranchAlreadyExists { branch } => {
                write!(
                    f,
                    "{ERROR_EMOJI} {ERROR}Branch {ERROR_BOLD}{branch}{ERROR_BOLD:#}{ERROR} already exists{ERROR:#}\n\n{HINT_EMOJI} {HINT}Remove --create flag to switch to it{HINT:#}"
                )
            }

            GitError::WorktreeMissing { branch } => {
                write!(
                    f,
                    "{ERROR_EMOJI} {ERROR}Worktree directory missing for {ERROR_BOLD}{branch}{ERROR_BOLD:#}{ERROR:#}\n\n{HINT_EMOJI} {HINT}Run 'git worktree prune' to clean up{HINT:#}"
                )
            }

            GitError::NoWorktreeFound { branch } => {
                write!(
                    f,
                    "{ERROR_EMOJI} {ERROR}No worktree found for branch {ERROR_BOLD}{branch}{ERROR_BOLD:#}{ERROR:#}"
                )
            }

            GitError::WorktreePathOccupied {
                branch,
                path,
                occupant,
            } => {
                let occupant_note = occupant
                    .as_ref()
                    .map(|b| format!(" (currently on {HINT_BOLD}{b}{HINT_BOLD:#}{HINT})"))
                    .unwrap_or_default();
                write!(
                    f,
                    "{ERROR_EMOJI} {ERROR}Cannot create worktree for {ERROR_BOLD}{branch}{ERROR_BOLD:#}{ERROR}: target path already exists{ERROR:#}\n\n{HINT_EMOJI} {HINT}Reuse the existing worktree at {}{} or remove it before retrying{HINT:#}",
                    format_path_for_display(path),
                    occupant_note
                )
            }

            GitError::WorktreePathExists { path } => {
                write!(
                    f,
                    "{ERROR_EMOJI} {ERROR}Directory already exists: {ERROR_BOLD}{}{ERROR_BOLD:#}{ERROR:#}\n\n{HINT_EMOJI} {HINT}Remove the directory or use a different branch name{HINT:#}",
                    format_path_for_display(path)
                )
            }

            GitError::WorktreeCreationFailed {
                branch,
                base_branch,
                error,
            } => {
                let base_suffix = base_branch
                    .as_ref()
                    .map(|base| {
                        format!("{ERROR} from base {ERROR_BOLD}{base}{ERROR_BOLD:#}{ERROR}")
                    })
                    .unwrap_or_default();
                let header = format!(
                    "{ERROR_EMOJI} {ERROR}Failed to create worktree for {ERROR_BOLD}{branch}{ERROR_BOLD:#}{base_suffix}{ERROR:#}"
                );
                write!(f, "{}", format_error_block(header, error))
            }

            GitError::WorktreeRemovalFailed {
                branch,
                path,
                error,
            } => {
                let header = format!(
                    "{ERROR_EMOJI} {ERROR}Failed to remove worktree for {ERROR_BOLD}{branch}{ERROR_BOLD:#}{ERROR} at {ERROR_BOLD}{}{ERROR_BOLD:#}{ERROR:#}",
                    format_path_for_display(path)
                );
                write!(f, "{}", format_error_block(header, error))
            }

            GitError::ConflictingChanges {
                files,
                worktree_path,
            } => {
                write!(
                    f,
                    "{ERROR_EMOJI} {ERROR}Cannot push: conflicting uncommitted changes in:{ERROR:#}\n\n"
                )?;
                if !files.is_empty() {
                    let joined_files = files.join("\n");
                    write!(f, "{}", format_with_gutter(&joined_files, "", None))?;
                }
                write!(
                    f,
                    "\n{HINT_EMOJI} {HINT}Commit or stash these changes in {} first{HINT:#}",
                    format_path_for_display(worktree_path)
                )
            }

            GitError::NotFastForward {
                target_branch,
                commits_formatted,
                in_merge_context,
            } => {
                write!(
                    f,
                    "{ERROR_EMOJI} {ERROR}Can't push to local {ERROR_BOLD}{target_branch}{ERROR_BOLD:#}{ERROR} branch: it has newer commits{ERROR:#}"
                )?;
                if !commits_formatted.is_empty() {
                    write!(f, "\n{}", format_with_gutter(commits_formatted, "", None))?;
                }
                // Context-appropriate hint
                let hint = if *in_merge_context {
                    "Run 'wt merge' again to incorporate these changes".to_string()
                } else {
                    format!("Use 'wt step rebase' or 'wt merge' to rebase onto {target_branch}")
                };
                write!(f, "\n{HINT_EMOJI} {HINT}{hint}{HINT:#}")
            }

            GitError::MergeCommitsFound => {
                write!(
                    f,
                    "{ERROR_EMOJI} {ERROR}Found merge commits in push range{ERROR:#}\n\n{HINT_EMOJI} {HINT}Use --allow-merge-commits to push non-linear history{HINT:#}"
                )
            }

            GitError::RebaseConflict {
                target_branch,
                git_output,
            } => {
                write!(
                    f,
                    "{ERROR_EMOJI} {ERROR}Rebase onto {ERROR_BOLD}{target_branch}{ERROR_BOLD:#}{ERROR} incomplete{ERROR:#}"
                )?;
                if !git_output.is_empty() {
                    write!(f, "\n{}", format_with_gutter(git_output, "", None))
                } else {
                    write!(
                        f,
                        "\n\n{HINT_EMOJI} {HINT}Resolve conflicts and run 'git rebase --continue'{HINT:#}\n{HINT_EMOJI} {HINT}Or abort with 'git rebase --abort'{HINT:#}"
                    )
                }
            }

            GitError::PushFailed { error } => {
                let header = format!("{ERROR_EMOJI} {ERROR}Push failed{ERROR:#}");
                write!(f, "{}", format_error_block(header, error))
            }

            GitError::NotInteractive => {
                write!(
                    f,
                    "{ERROR_EMOJI} {ERROR}Cannot prompt for approval in non-interactive environment{ERROR:#}\n\n{HINT_EMOJI} {HINT}In CI/CD, use --force to skip prompts. To pre-approve commands, use 'wt config approvals add'{HINT:#}"
                )
            }

            GitError::LlmCommandFailed { command, error } => {
                let error_header =
                    format!("{ERROR_EMOJI} {ERROR}Commit generation command failed{ERROR:#}");
                let error_block = format_error_block(error_header, error);
                let command_gutter = format_with_gutter(command, "", None);
                write!(
                    f,
                    "{}\n\n{INFO_EMOJI} Ran command:\n{}",
                    error_block.trim_end(),
                    command_gutter.trim_end()
                )
            }

            GitError::ProjectConfigNotFound { config_path } => {
                write!(
                    f,
                    "{ERROR_EMOJI} {ERROR}No project configuration found{ERROR:#}\n\n{HINT_EMOJI} {HINT}Create a config file at: {HINT_BOLD}{}{HINT_BOLD:#}{HINT:#}",
                    format_path_for_display(config_path)
                )
            }

            GitError::ParseError { message } | GitError::Other { message } => {
                write!(f, "{ERROR_EMOJI} {ERROR}{message}{ERROR:#}")
            }
        }
    }
}

// =============================================================================
// WorktrunkError - Semantic errors with special handling
// =============================================================================

/// Semantic errors that require special handling in main.rs
///
/// Most errors use anyhow::bail! with formatted messages. This enum is only
/// for cases that need exit code extraction or special handling.
#[derive(Debug)]
pub enum WorktrunkError {
    /// Child process exited with non-zero code (preserves exit code for signals)
    ChildProcessExited { code: i32, message: String },
    /// Hook command failed
    HookCommandFailed {
        hook_type: HookType,
        command_name: Option<String>,
        error: String,
        exit_code: Option<i32>,
    },
    /// Command was not approved by user (silent error)
    CommandNotApproved,
}

impl std::fmt::Display for WorktrunkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorktrunkError::ChildProcessExited { message, .. } => {
                write!(f, "{ERROR_EMOJI} {ERROR}{message}{ERROR:#}")
            }
            WorktrunkError::HookCommandFailed {
                hook_type,
                command_name,
                error,
                ..
            } => {
                let name_suffix = command_name
                    .as_ref()
                    .map(|n| format!(": {ERROR_BOLD}{n}{ERROR_BOLD:#}{ERROR}"))
                    .unwrap_or_default();

                write!(
                    f,
                    "{ERROR_EMOJI} {ERROR}{hook_type} command failed{name_suffix}: {error}{ERROR:#}\n\n{HINT_EMOJI} {HINT}Use --no-verify to skip {hook_type} commands{HINT:#}"
                )
            }
            WorktrunkError::CommandNotApproved => {
                Ok(()) // on_skip callback handles the printing
            }
        }
    }
}

impl std::error::Error for WorktrunkError {}

/// Extract exit code from WorktrunkError, if applicable
pub fn exit_code(err: &anyhow::Error) -> Option<i32> {
    err.downcast_ref::<WorktrunkError>().and_then(|e| match e {
        WorktrunkError::ChildProcessExited { code, .. } => Some(*code),
        WorktrunkError::HookCommandFailed { exit_code, .. } => *exit_code,
        WorktrunkError::CommandNotApproved => None,
    })
}

/// Check if error is CommandNotApproved (silent error)
pub fn is_command_not_approved(err: &anyhow::Error) -> bool {
    err.downcast_ref::<WorktrunkError>()
        .is_some_and(|e| matches!(e, WorktrunkError::CommandNotApproved))
}

// =============================================================================
// Error formatting helpers
// =============================================================================

/// Format an error with header and gutter content
fn format_error_block(header: String, error: &str) -> String {
    let trimmed = error.trim();
    if trimmed.is_empty() {
        header
    } else {
        format!("{header}\n{}", format_with_gutter(trimmed, "", None))
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_error_display_contains_emoji() {
        // Display produces styled output with emoji
        let err = GitError::DetachedHead { action: None };
        let output = err.to_string();
        assert!(output.contains("❌")); // ERROR_EMOJI
        assert!(output.contains("detached HEAD"));
        assert!(output.contains("💡")); // HINT_EMOJI
    }

    #[test]
    fn test_git_error_display_includes_action() {
        let err = GitError::DetachedHead {
            action: Some("push".into()),
        };
        let output = err.to_string();
        assert!(output.contains("Cannot push"));

        let err = GitError::UncommittedChanges {
            action: Some("remove worktree".into()),
        };
        let output = err.to_string();
        assert!(output.contains("Cannot remove worktree"));
    }

    #[test]
    fn test_into_preserves_type_for_display() {
        // .into() preserves type so we can downcast and use Display
        let err: anyhow::Error = GitError::BranchAlreadyExists {
            branch: "main".into(),
        }
        .into();

        // Can downcast and get styled output via Display
        let git_err = err.downcast_ref::<GitError>().expect("Should downcast");
        let output = git_err.to_string();
        assert!(output.contains("❌")); // Should be styled
        assert!(output.contains("main"));
        assert!(output.contains("already exists"));
    }

    #[test]
    fn test_pattern_matching_with_into() {
        // .into() preserves type for pattern matching
        let err: anyhow::Error = GitError::BranchAlreadyExists {
            branch: "main".into(),
        }
        .into();

        if let Some(GitError::BranchAlreadyExists { branch }) = err.downcast_ref::<GitError>() {
            assert_eq!(branch, "main");
        } else {
            panic!("Failed to downcast and pattern match");
        }
    }

    #[test]
    fn test_worktree_error_with_path() {
        let err = GitError::WorktreePathExists {
            path: PathBuf::from("/some/path"),
        };
        let output = err.to_string();
        assert!(output.contains("Directory already exists"));
    }
}
