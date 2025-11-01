use crate::display::{find_common_prefix, get_terminal_width};
use std::path::{Path, PathBuf};
use unicode_width::UnicodeWidthStr;

use super::model::ListItem;

/// Helper: Try to allocate space for a column. Returns the allocated width if successful.
/// Updates `remaining` by subtracting the allocated width + spacing.
/// If is_first is true, doesn't require spacing before the column.
///
/// The spacing is consumed from the budget (subtracted from `remaining`) but not returned
/// as part of the column's width, since the spacing appears before the column content.
fn try_allocate(
    remaining: &mut usize,
    ideal_width: usize,
    spacing: usize,
    is_first: bool,
) -> usize {
    if ideal_width == 0 {
        return 0;
    }
    let required = if is_first {
        ideal_width
    } else {
        ideal_width + spacing // Gap before column + column content
    };
    if *remaining < required {
        return 0;
    }
    *remaining = remaining.saturating_sub(required);
    ideal_width // Return just the column width
}

/// Width information for diff columns (e.g., "+128 -147")
#[derive(Clone, Copy, Debug)]
pub struct DiffWidths {
    pub total: usize,
    pub added_digits: usize,
    pub deleted_digits: usize,
}

impl DiffWidths {
    pub fn zero() -> Self {
        Self {
            total: 0,
            added_digits: 0,
            deleted_digits: 0,
        }
    }
}

pub struct ColumnWidths {
    pub branch: usize,
    pub time: usize,
    pub message: usize,
    pub ahead_behind: DiffWidths,
    pub working_diff: DiffWidths,
    pub branch_diff: DiffWidths,
    pub upstream: DiffWidths,
    pub states: usize,
    pub commit: usize,
    pub path: usize,
}

pub struct LayoutConfig {
    pub widths: ColumnWidths,
    pub common_prefix: PathBuf,
    pub max_message_len: usize,
}

pub fn calculate_column_widths(items: &[ListItem]) -> ColumnWidths {
    // Initialize with header label widths to ensure headers always fit
    let mut max_branch = "Branch".width();
    let mut max_time = "Age".width();
    let mut max_message = "Message".width();
    let mut max_states = 0; // Start at 0, will use header width if needed

    // Track diff component widths separately
    let mut max_wt_added_digits = 0;
    let mut max_wt_deleted_digits = 0;
    let mut max_br_added_digits = 0;
    let mut max_br_deleted_digits = 0;

    // Track ahead/behind digit widths separately for alignment
    let mut max_ahead_digits = 0;
    let mut max_behind_digits = 0;
    let mut max_upstream_ahead_digits = 0;
    let mut max_upstream_behind_digits = 0;

    for item in items {
        let commit = item.commit_details();
        let counts = item.counts();
        let branch_diff = item.branch_diff().diff;
        let upstream = item.upstream();
        let worktree_info = item.worktree_info();

        // Branch name
        max_branch = max_branch.max(item.branch_name().width());

        // Time
        let time_str = crate::display::format_relative_time(commit.timestamp);
        max_time = max_time.max(time_str.width());

        // Message (truncate to 50 chars max)
        let msg_len = commit.commit_message.chars().take(50).count();
        max_message = max_message.max(msg_len);

        // Ahead/behind (only for non-primary items) - track digits separately
        if !item.is_primary() && (counts.ahead > 0 || counts.behind > 0) {
            max_ahead_digits = max_ahead_digits.max(counts.ahead.to_string().len());
            max_behind_digits = max_behind_digits.max(counts.behind.to_string().len());
        }

        // Working tree diff (worktrees only) - track digits separately
        if let Some(info) = worktree_info
            && (info.working_tree_diff.0 > 0 || info.working_tree_diff.1 > 0)
        {
            max_wt_added_digits =
                max_wt_added_digits.max(info.working_tree_diff.0.to_string().len());
            max_wt_deleted_digits =
                max_wt_deleted_digits.max(info.working_tree_diff.1.to_string().len());
        }

        // Branch diff (only for non-primary items) - track digits separately
        if !item.is_primary() && (branch_diff.0 > 0 || branch_diff.1 > 0) {
            max_br_added_digits = max_br_added_digits.max(branch_diff.0.to_string().len());
            max_br_deleted_digits = max_br_deleted_digits.max(branch_diff.1.to_string().len());
        }

        // Upstream tracking - track digits separately
        if let Some((_remote_name, upstream_ahead, upstream_behind)) = upstream.active() {
            max_upstream_ahead_digits =
                max_upstream_ahead_digits.max(upstream_ahead.to_string().len());
            max_upstream_behind_digits =
                max_upstream_behind_digits.max(upstream_behind.to_string().len());
        }

        // States (worktrees only)
        if let Some(info) = worktree_info {
            let states = super::render::format_all_states(info);
            if !states.is_empty() {
                max_states = max_states.max(states.width());
            }
        }
    }

    // Calculate diff widths: "+{added} -{deleted}"
    // Format: "+" + digits + " " + "-" + digits
    let working_diff_total = if max_wt_added_digits > 0 || max_wt_deleted_digits > 0 {
        let data_width = 1 + max_wt_added_digits + 1 + 1 + max_wt_deleted_digits;
        data_width.max("WT +/-".width()) // Ensure header fits if we have data
    } else {
        0 // No data, no column
    };
    let branch_diff_total = if max_br_added_digits > 0 || max_br_deleted_digits > 0 {
        let data_width = 1 + max_br_added_digits + 1 + 1 + max_br_deleted_digits;
        data_width.max("Branch +/-".width()) // Ensure header fits if we have data
    } else {
        0 // No data, no column
    };

    // Calculate ahead/behind column width (format: "↑n ↓n")
    let ahead_behind_total = if max_ahead_digits > 0 || max_behind_digits > 0 {
        let data_width = 1 + max_ahead_digits + 1 + 1 + max_behind_digits;
        data_width.max("Commits".width())
    } else {
        0
    };

    // Calculate upstream column width (format: "↑n ↓n" or "remote ↑n ↓n")
    let upstream_total = if max_upstream_ahead_digits > 0 || max_upstream_behind_digits > 0 {
        let data_width = 1 + max_upstream_ahead_digits + 1 + 1 + max_upstream_behind_digits;
        data_width.max("Remote".width())
    } else {
        0
    };

    let final_states = if max_states > 0 {
        max_states.max("State".width())
    } else {
        0
    };

    ColumnWidths {
        branch: max_branch,
        time: max_time,
        message: max_message,
        ahead_behind: DiffWidths {
            total: ahead_behind_total,
            added_digits: max_ahead_digits,
            deleted_digits: max_behind_digits,
        },
        working_diff: DiffWidths {
            total: working_diff_total,
            added_digits: max_wt_added_digits,
            deleted_digits: max_wt_deleted_digits,
        },
        branch_diff: DiffWidths {
            total: branch_diff_total,
            added_digits: max_br_added_digits,
            deleted_digits: max_br_deleted_digits,
        },
        upstream: DiffWidths {
            total: upstream_total,
            added_digits: max_upstream_ahead_digits,
            deleted_digits: max_upstream_behind_digits,
        },
        states: final_states,
        commit: 8, // Fixed width for short commit hash
        path: 0,   // Path width calculated later in responsive layout
    }
}

/// Calculate responsive layout based on terminal width
pub fn calculate_responsive_layout(items: &[ListItem]) -> LayoutConfig {
    let terminal_width = get_terminal_width();
    let paths: Vec<&Path> = items
        .iter()
        .filter_map(|item| item.worktree_path().map(|path| path.as_path()))
        .collect();
    let common_prefix = find_common_prefix(&paths);

    // Calculate ideal column widths
    let ideal_widths = calculate_column_widths(items);

    // Calculate actual maximum path width (after common prefix removal)
    let max_path_width = items
        .iter()
        .filter_map(|item| item.worktree_path())
        .map(|path| {
            use crate::display::shorten_path;
            use unicode_width::UnicodeWidthStr;
            shorten_path(path.as_path(), &common_prefix).width()
        })
        .max()
        .unwrap_or(20); // fallback to 20 if no paths

    let spacing = 2;
    let commit_width = 8; // Short commit hash

    // Priority order for columns (from high to low):
    // 1. branch - identity (what is this?)
    // 2. working_diff - uncommitted changes (CRITICAL: do I need to commit?)
    // 3. ahead_behind - commits difference (CRITICAL: am I ahead/behind?)
    // 4. branch_diff - line diff in commits (work volume in those commits)
    // 5. states - special states like [rebasing] (rare but urgent when present)
    // 6. path - location (where is this?)
    // 7. upstream - tracking configuration (sync context)
    // 8. time - recency (nice-to-have context)
    // 9. commit - hash (reference info, rarely needed)
    // 10. message - description (nice-to-have, space-hungry)
    //
    // Note: ahead_behind and branch_diff are adjacent (both describe commits vs main)
    // Each column is shown if it has any data (ideal_width > 0) and fits in remaining space.
    // All columns participate in priority allocation - nothing is "essential".

    let mut remaining = terminal_width;
    let mut widths = ColumnWidths {
        branch: 0,
        time: 0,
        message: 0,
        ahead_behind: DiffWidths::zero(),
        working_diff: DiffWidths::zero(),
        branch_diff: DiffWidths::zero(),
        upstream: DiffWidths::zero(),
        states: 0,
        commit: 0,
        path: 0,
    };

    // Branch column (highest priority - identity)
    widths.branch = try_allocate(&mut remaining, ideal_widths.branch, spacing, true);

    // Working diff column (critical - uncommitted changes)
    let allocated_width = try_allocate(
        &mut remaining,
        ideal_widths.working_diff.total,
        spacing,
        false,
    );
    if allocated_width > 0 {
        widths.working_diff = ideal_widths.working_diff;
    }

    // Ahead/behind column (critical sync status)
    let allocated_width = try_allocate(
        &mut remaining,
        ideal_widths.ahead_behind.total,
        spacing,
        false,
    );
    if allocated_width > 0 {
        widths.ahead_behind = ideal_widths.ahead_behind;
    }

    // Branch diff column (work volume in those commits)
    let allocated_width = try_allocate(
        &mut remaining,
        ideal_widths.branch_diff.total,
        spacing,
        false,
    );
    if allocated_width > 0 {
        widths.branch_diff = ideal_widths.branch_diff;
    }

    // States column (rare but urgent when present)
    widths.states = try_allocate(&mut remaining, ideal_widths.states, spacing, false);

    // Path column (location - important for navigation)
    widths.path = try_allocate(&mut remaining, max_path_width, spacing, false);

    // Upstream column (sync configuration)
    let allocated_width = try_allocate(&mut remaining, ideal_widths.upstream.total, spacing, false);
    if allocated_width > 0 {
        widths.upstream = ideal_widths.upstream;
    }

    // Time column (contextual information)
    widths.time = try_allocate(&mut remaining, ideal_widths.time, spacing, false);

    // Commit column (reference hash - rarely needed)
    widths.commit = try_allocate(&mut remaining, commit_width, spacing, false);

    // Message column (flexible width: min 20, preferred 50, max 100)
    const MIN_MESSAGE: usize = 20;
    const PREFERRED_MESSAGE: usize = 50;
    const MAX_MESSAGE: usize = 100;

    let message_width = if remaining >= PREFERRED_MESSAGE + spacing {
        PREFERRED_MESSAGE
    } else if remaining >= MIN_MESSAGE + spacing {
        remaining.saturating_sub(spacing).min(ideal_widths.message)
    } else {
        0
    };

    if message_width > 0 {
        remaining = remaining.saturating_sub(message_width + spacing);
        widths.message = message_width.min(ideal_widths.message);

        // Expand with any leftover space (up to MAX_MESSAGE total)
        if remaining > 0 {
            let expansion = remaining.min(MAX_MESSAGE.saturating_sub(widths.message));
            widths.message += expansion;
        }
    }

    let final_max_message_len = widths.message;

    LayoutConfig {
        widths,
        common_prefix,
        max_message_len: final_max_message_len,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_column_width_calculation_with_unicode() {
        use crate::commands::list::model::{
            AheadBehind, BranchDiffTotals, CommitDetails, UpstreamStatus, WorktreeInfo,
        };

        let info1 = WorktreeInfo {
            worktree: worktrunk::git::Worktree {
                path: PathBuf::from("/test"),
                head: "abc123".to_string(),
                branch: Some("main".to_string()),
                bare: false,
                detached: false,
                locked: None,
                prunable: None,
            },
            commit: CommitDetails {
                timestamp: 0,
                commit_message: "Test".to_string(),
            },
            counts: AheadBehind {
                ahead: 3,
                behind: 2,
            },
            working_tree_diff: (100, 50),
            branch_diff: BranchDiffTotals { diff: (200, 30) },
            is_primary: false,
            upstream: UpstreamStatus::from_parts(Some("origin".to_string()), 4, 0),
            worktree_state: None,
        };

        let widths = calculate_column_widths(&[super::ListItem::Worktree(info1)]);

        // "↑3 ↓2" has format "↑3 ↓2" = 1+1+1+1+1 = 5, but header "Commits" is 7
        assert_eq!(
            widths.ahead_behind.total, 7,
            "Ahead/behind column should fit header 'Commits' (width 7)"
        );
        assert_eq!(widths.ahead_behind.added_digits, 1, "3 has 1 digit");
        assert_eq!(widths.ahead_behind.deleted_digits, 1, "2 has 1 digit");

        // "+100 -50" has width 8
        assert_eq!(widths.working_diff.total, 8, "+100 -50 should have width 8");
        assert_eq!(widths.working_diff.added_digits, 3, "100 has 3 digits");
        assert_eq!(widths.working_diff.deleted_digits, 2, "50 has 2 digits");

        // "+200 -30" has width 8, but header "Branch +/-" is 10, so column width is 10
        assert_eq!(
            widths.branch_diff.total, 10,
            "Branch diff column should fit header 'Branch +/-' (width 10)"
        );
        assert_eq!(widths.branch_diff.added_digits, 3, "200 has 3 digits");
        assert_eq!(widths.branch_diff.deleted_digits, 2, "30 has 2 digits");

        // Single remote: "↑4 ↓0" has format "↑4 ↓0" = 1+1+1+1+1 = 5, but header "Remote" is 6
        assert_eq!(
            widths.upstream.total, 6,
            "Upstream column should fit header 'Remote' (width 6) when single remote"
        );
        assert_eq!(widths.upstream.added_digits, 1, "4 has 1 digit");
        assert_eq!(widths.upstream.deleted_digits, 1, "0 has 1 digit");
    }
}
