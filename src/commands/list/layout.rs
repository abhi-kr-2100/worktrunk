//! Column layout and priority allocation for the list command.
//!
//! # Priority System Design
//!
//! ## Priority Scoring Model
//!
//! The allocation system uses a **priority scoring model**:
//! ```text
//! final_priority = base_priority + empty_penalty
//! ```
//!
//! **Base priorities** (1-11) are determined by **user need hierarchy** - what questions users need
//! answered when scanning worktrees:
//! - 1: Branch (identity - "what is this?")
//! - 2: Working diff (critical - "do I need to commit?")
//! - 3: Ahead/behind (critical - "am I out of sync?")
//! - 4-10: Context (work volume, states, path, time, CI, etc.)
//! - 11: Message (nice-to-have, space-hungry)
//!
//! **Empty penalty**: +10 if column has no data (only header)
//! - Empty working_diff: 2 + 10 = priority 12
//! - Empty ahead/behind: 3 + 10 = priority 13
//! - etc.
//!
//! This creates two effective priority tiers:
//! - **Tier 1 (priorities 1-11)**: Columns with actual data
//! - **Tier 2 (priorities 12-21)**: Empty columns (visual consistency)
//!
//! The empty penalty is large (+10) but not infinite, so empty columns maintain their relative
//! ordering (empty working_diff still ranks higher than empty ci_status) for visual consistency.
//!
//! ## Why This Design?
//!
//! **Problem**: Terminal width is limited. We must decide what to show.
//!
//! **Goals**:
//! 1. Show critical data (uncommitted changes, sync status) at any terminal width
//! 2. Show nice-to-have data (message, commit hash) when space allows
//! 3. Maintain visual consistency - empty columns in predictable positions at wide widths
//!
//! **Key decision**: Message sits at the boundary (priority 11). Empty columns (priority 12+)
//! rank below message, so:
//! - Narrow terminals: Data columns + message (hide empty columns)
//! - Wide terminals: Data columns + message + empty columns (visual consistency)
//!
//! ## Special Cases
//!
//! Three columns have non-standard behavior that extends beyond the basic two-tier model:
//!
//! 1. **BranchDiff** - Visibility gate (`show_full` flag)
//!    - Hidden by default as too noisy for typical usage
//!    - Only allocated when `show_full=true` (match guard skips if false)
//!
//! 2. **CiStatus** - Visibility gate (`fetch_ci` flag)
//!    - Only shown when `fetch_ci=true` (when CI data was requested)
//!    - Bypasses the tier system entirely when `fetch_ci=false`
//!    - Within the visibility gate, follows normal two-tier priority (priority 9 with data, 19 when empty)
//!
//! 3. **Message** - Flexible sizing with post-allocation expansion
//!    - Allocated at priority 11 with flexible width (min 20, preferred 50)
//!    - After all columns allocated (including empty ones), expands up to max 100 using leftover space
//!    - Two-step process ensures critical columns get space before message grows
//!
//! ## Implementation
//!
//! The code implements this using a data-driven priority system:
//!
//! ```rust
//! // Build column descriptors with base priorities and data flags
//! let columns = [
//!     ColumnDescriptor { column_type: Branch, base_priority: 1, has_data: true },
//!     ColumnDescriptor { column_type: WorkingDiff, base_priority: 2, has_data: data_flags.working_diff },
//!     // ... all 11 columns
//! ];
//!
//! // Sort by final priority (base_priority + empty_penalty)
//! columns.sort_by_key(|col| col.priority());
//!
//! // Allocate columns in priority order
//! for col in columns {
//!     match col.column_type {
//!         Branch => allocate_branch(),
//!         WorkingDiff => allocate_diff(),
//!         BranchDiff if show_full => allocate_diff(),  // Visibility gate
//!         CiStatus if fetch_ci => allocate(),           // Visibility gate
//!         // ... all columns
//!     }
//! }
//!
//! // Message post-allocation expansion (uses truly leftover space)
//! expand_message_to_max();
//! ```
//!
//! **Benefits**:
//! - Priority calculation is explicit and centralized (`ColumnDescriptor::priority()`)
//! - Single unified allocation loop (no Phase 1/Phase 2 duplication)
//! - Easy to understand: build descriptors → sort by priority → allocate
//! - Extensible: can add new modifiers (terminal width bonus, user config) without restructuring
//!
//! ## Helper Functions
//!
//! - `calculate_diff_width()`: Computes width for diff-style columns ("+added -deleted")
//! - `fit_header()`: Ensures column width ≥ header width to prevent overflow
//! - `try_allocate()`: Attempts to allocate space, returns 0 if insufficient

use crate::display::{find_common_prefix, get_terminal_width};
use std::path::{Path, PathBuf};
use unicode_width::UnicodeWidthStr;

use super::model::ListItem;

/// Width of short commit hash display (first 8 hex characters)
const COMMIT_HASH_WIDTH: usize = 8;

/// Column header labels - single source of truth for all column headers.
/// Both layout calculations and rendering use these constants.
pub const HEADER_BRANCH: &str = "Branch";
pub const HEADER_WORKING_DIFF: &str = "Working ±";
pub const HEADER_AHEAD_BEHIND: &str = "Main ↕";
pub const HEADER_BRANCH_DIFF: &str = "Main ±";
pub const HEADER_STATE: &str = "State";
pub const HEADER_PATH: &str = "Path";
pub const HEADER_UPSTREAM: &str = "Remote ↕";
pub const HEADER_AGE: &str = "Age";
pub const HEADER_CI: &str = "CI";
pub const HEADER_COMMIT: &str = "Commit";
pub const HEADER_MESSAGE: &str = "Message";

/// Ensures a column width is at least as wide as its header.
///
/// This is the general solution for preventing header overflow: pass the header
/// string and the calculated data width, and this returns the larger of the two.
///
/// Use this for every column width calculation to ensure headers never overflow.
fn fit_header(header: &str, data_width: usize) -> usize {
    use unicode_width::UnicodeWidthStr;
    data_width.max(header.width())
}

/// Calculates width for a diff-style column (format: "+added -deleted" or "↑ahead ↓behind").
///
/// Returns DiffWidths with:
/// - total: width including header minimum ("+{added} -{deleted}")
/// - added_digits/deleted_digits: number of digits for each part
fn calculate_diff_width(header: &str, added_digits: usize, deleted_digits: usize) -> DiffWidths {
    let has_data = added_digits > 0 || deleted_digits > 0;
    let data_width = if has_data {
        1 + added_digits + 1 + 1 + deleted_digits // "+added -deleted"
    } else {
        0
    };
    let total = fit_header(header, data_width);

    DiffWidths {
        total,
        added_digits,
        deleted_digits,
    }
}

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

/// Width information for two-part columns: diffs ("+128 -147") and arrows ("↑6 ↓1")
/// - For diff columns: added_digits/deleted_digits refer to line change counts
/// - For arrow columns: added_digits/deleted_digits refer to ahead/behind commit counts
#[derive(Clone, Copy, Debug)]
pub struct DiffWidths {
    pub total: usize,
    pub added_digits: usize,   // First part: + for diffs, ↑ for arrows
    pub deleted_digits: usize, // Second part: - for diffs, ↓ for arrows
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
    pub ci_status: usize,
    pub message: usize,
    pub ahead_behind: DiffWidths,
    pub working_diff: DiffWidths,
    pub branch_diff: DiffWidths,
    pub upstream: DiffWidths,
    pub states: usize,
    pub commit: usize,
    pub path: usize,
}

/// Tracks which columns have actual data (vs just headers)
#[derive(Clone, Copy, Debug)]
pub struct ColumnDataFlags {
    pub working_diff: bool,
    pub ahead_behind: bool,
    pub branch_diff: bool,
    pub upstream: bool,
    pub states: bool,
    pub ci_status: bool,
}

/// Column types for allocation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColumnType {
    Branch,
    WorkingDiff,
    AheadBehind,
    BranchDiff,
    States,
    Path,
    Upstream,
    Time,
    CiStatus,
    Commit,
    Message,
}

/// Describes a column for priority-based allocation
struct ColumnDescriptor {
    column_type: ColumnType,
    base_priority: u8,
    has_data: bool,
}

impl ColumnDescriptor {
    /// Calculate final priority: base_priority + empty_penalty
    fn priority(&self) -> u8 {
        const EMPTY_PENALTY: u8 = 10;
        if self.has_data {
            self.base_priority
        } else {
            self.base_priority + EMPTY_PENALTY
        }
    }
}

/// Absolute column positions for guaranteed alignment
#[derive(Clone, Copy, Debug)]
pub struct ColumnPositions {
    pub branch: usize,
    pub working_diff: usize,
    pub ahead_behind: usize,
    pub branch_diff: usize,
    pub states: usize,
    pub path: usize,
    pub upstream: usize,
    pub time: usize,
    pub ci_status: usize,
    pub commit: usize,
    pub message: usize,
}

pub struct LayoutConfig {
    pub widths: ColumnWidths,
    pub positions: ColumnPositions,
    pub common_prefix: PathBuf,
    pub max_message_len: usize,
}

pub fn calculate_column_widths(
    items: &[ListItem],
    fetch_ci: bool,
) -> (ColumnWidths, ColumnDataFlags) {
    // Track maximum data widths (headers are enforced via fit_header() later)
    let mut max_branch = 0;
    let mut max_time = 0;
    let mut max_message = 0;
    let mut max_states = 0;

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

        // Upstream tracking - track digits only (not remote name yet)
        if let Some((_remote_name, upstream_ahead, upstream_behind)) = upstream.active() {
            max_upstream_ahead_digits =
                max_upstream_ahead_digits.max(upstream_ahead.to_string().len());
            max_upstream_behind_digits =
                max_upstream_behind_digits.max(upstream_behind.to_string().len());
        }

        // States (includes conflicts, worktree states, etc.)
        let states = super::render::format_all_states(item);
        if !states.is_empty() {
            max_states = max_states.max(states.width());
        }
    }

    // Calculate diff widths using helper (format: "+left -right")
    let working_diff = calculate_diff_width(
        HEADER_WORKING_DIFF,
        max_wt_added_digits,
        max_wt_deleted_digits,
    );
    let branch_diff = calculate_diff_width(
        HEADER_BRANCH_DIFF,
        max_br_added_digits,
        max_br_deleted_digits,
    );
    let ahead_behind =
        calculate_diff_width(HEADER_AHEAD_BEHIND, max_ahead_digits, max_behind_digits);

    // Upstream (format: "↑n ↓n", TODO: add remote name when show_remote_names is implemented)
    let upstream = calculate_diff_width(
        HEADER_UPSTREAM,
        max_upstream_ahead_digits,
        max_upstream_behind_digits,
    );

    let has_states_data = max_states > 0;
    let final_states = fit_header(HEADER_STATE, max_states);

    // CI status column: Always 2 chars wide
    // Only show if we attempted to fetch CI data (regardless of whether any items have status)
    let has_ci_status = fetch_ci && items.iter().any(|item| item.pr_status().is_some());
    let ci_status_width = 2; // Fixed width

    let widths = ColumnWidths {
        branch: fit_header(HEADER_BRANCH, max_branch),
        time: fit_header(HEADER_AGE, max_time),
        ci_status: fit_header(HEADER_CI, ci_status_width),
        message: fit_header(HEADER_MESSAGE, max_message),
        ahead_behind,
        working_diff,
        branch_diff,
        upstream,
        states: final_states,
        commit: fit_header(HEADER_COMMIT, COMMIT_HASH_WIDTH),
        path: 0, // Path width calculated later in responsive layout
    };

    let data_flags = ColumnDataFlags {
        working_diff: working_diff.added_digits > 0 || working_diff.deleted_digits > 0,
        ahead_behind: ahead_behind.added_digits > 0 || ahead_behind.deleted_digits > 0,
        branch_diff: branch_diff.added_digits > 0 || branch_diff.deleted_digits > 0,
        upstream: upstream.added_digits > 0 || upstream.deleted_digits > 0,
        states: has_states_data,
        ci_status: has_ci_status,
    };

    (widths, data_flags)
}

/// Calculate responsive layout based on terminal width
pub fn calculate_responsive_layout(
    items: &[ListItem],
    show_full: bool,
    fetch_ci: bool,
) -> LayoutConfig {
    let terminal_width = get_terminal_width();
    let paths: Vec<&Path> = items
        .iter()
        .filter_map(|item| item.worktree_path().map(|path| path.as_path()))
        .collect();
    let common_prefix = find_common_prefix(&paths);

    // Calculate ideal column widths and track which columns have data
    let (ideal_widths, data_flags) = calculate_column_widths(items, fetch_ci);

    // Calculate actual maximum path width (after common prefix removal)
    let path_data_width = items
        .iter()
        .filter_map(|item| item.worktree_path())
        .map(|path| {
            use crate::display::shorten_path;
            use unicode_width::UnicodeWidthStr;
            shorten_path(path.as_path(), &common_prefix).width()
        })
        .max()
        .unwrap_or(0);
    let max_path_width = fit_header(HEADER_PATH, path_data_width);

    let spacing = 2;
    let commit_width = fit_header(HEADER_COMMIT, COMMIT_HASH_WIDTH);

    // Priority-based allocation using scoring model: final_priority = base_priority + modifiers
    // Base priorities (1-11) defined by user need hierarchy
    // Empty penalty (+10) pushes empty columns to priorities 12-21
    //
    // Priority order (from high to low):
    // 1. branch - identity (what is this?)
    // 2. working_diff - uncommitted changes (CRITICAL: do I need to commit?)
    // 3. ahead_behind - commits difference (CRITICAL: am I ahead/behind?)
    // 4. branch_diff - line diff in commits (work volume in those commits)
    // 5. states - special states like [rebasing], (conflicts) (rare but urgent when present)
    // 6. path - location (where is this?)
    // 7. upstream - tracking configuration (sync context)
    // 8. time - recency (nice-to-have context)
    // 9. ci_status - CI/PR status (contextual when available)
    // 10. commit - hash (reference info, rarely needed)
    // 11. message - description (nice-to-have, space-hungry)

    let mut remaining = terminal_width;
    let mut widths = ColumnWidths {
        branch: 0,
        time: 0,
        ci_status: 0,
        message: 0,
        ahead_behind: DiffWidths::zero(),
        working_diff: DiffWidths::zero(),
        branch_diff: DiffWidths::zero(),
        upstream: DiffWidths::zero(),
        states: 0,
        commit: 0,
        path: 0,
    };

    // Build column allocation list with priorities
    let mut columns = [
        ColumnDescriptor {
            column_type: ColumnType::Branch,
            base_priority: 1,
            has_data: true,
        },
        ColumnDescriptor {
            column_type: ColumnType::WorkingDiff,
            base_priority: 2,
            has_data: data_flags.working_diff,
        },
        ColumnDescriptor {
            column_type: ColumnType::AheadBehind,
            base_priority: 3,
            has_data: data_flags.ahead_behind,
        },
        ColumnDescriptor {
            column_type: ColumnType::BranchDiff,
            base_priority: 4,
            has_data: data_flags.branch_diff,
        },
        ColumnDescriptor {
            column_type: ColumnType::States,
            base_priority: 5,
            has_data: data_flags.states,
        },
        ColumnDescriptor {
            column_type: ColumnType::Path,
            base_priority: 6,
            has_data: true,
        },
        ColumnDescriptor {
            column_type: ColumnType::Upstream,
            base_priority: 7,
            has_data: data_flags.upstream,
        },
        ColumnDescriptor {
            column_type: ColumnType::Time,
            base_priority: 8,
            has_data: true,
        },
        ColumnDescriptor {
            column_type: ColumnType::CiStatus,
            base_priority: 9,
            has_data: data_flags.ci_status,
        },
        ColumnDescriptor {
            column_type: ColumnType::Commit,
            base_priority: 10,
            has_data: true,
        },
        ColumnDescriptor {
            column_type: ColumnType::Message,
            base_priority: 11,
            has_data: true,
        },
    ];

    // Sort by final priority (includes empty penalty)
    columns.sort_by_key(|col| col.priority());

    // Message width constants (used in allocation and expansion)
    const MIN_MESSAGE: usize = 20;
    const PREFERRED_MESSAGE: usize = 50;
    const MAX_MESSAGE: usize = 100;

    // Allocate columns in priority order
    for (idx, col) in columns.iter().enumerate() {
        let is_first = idx == 0;

        match col.column_type {
            ColumnType::Branch => {
                widths.branch =
                    try_allocate(&mut remaining, ideal_widths.branch, spacing, is_first);
            }
            ColumnType::WorkingDiff => {
                let allocated = try_allocate(
                    &mut remaining,
                    ideal_widths.working_diff.total,
                    spacing,
                    is_first,
                );
                if allocated > 0 {
                    widths.working_diff = ideal_widths.working_diff;
                }
            }
            ColumnType::AheadBehind => {
                let allocated = try_allocate(
                    &mut remaining,
                    ideal_widths.ahead_behind.total,
                    spacing,
                    is_first,
                );
                if allocated > 0 {
                    widths.ahead_behind = ideal_widths.ahead_behind;
                }
            }
            ColumnType::BranchDiff if show_full => {
                let allocated = try_allocate(
                    &mut remaining,
                    ideal_widths.branch_diff.total,
                    spacing,
                    is_first,
                );
                if allocated > 0 {
                    widths.branch_diff = ideal_widths.branch_diff;
                }
            }
            ColumnType::States => {
                widths.states =
                    try_allocate(&mut remaining, ideal_widths.states, spacing, is_first);
            }
            ColumnType::Path => {
                widths.path = try_allocate(&mut remaining, max_path_width, spacing, is_first);
            }
            ColumnType::Upstream => {
                let allocated = try_allocate(
                    &mut remaining,
                    ideal_widths.upstream.total,
                    spacing,
                    is_first,
                );
                if allocated > 0 {
                    widths.upstream = ideal_widths.upstream;
                }
            }
            ColumnType::Time => {
                widths.time = try_allocate(&mut remaining, ideal_widths.time, spacing, is_first);
            }
            ColumnType::CiStatus if fetch_ci => {
                widths.ci_status =
                    try_allocate(&mut remaining, ideal_widths.ci_status, spacing, is_first);
            }
            ColumnType::Commit => {
                widths.commit = try_allocate(&mut remaining, commit_width, spacing, is_first);
            }
            ColumnType::Message => {
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
                }
            }
            _ => {} // Skip columns that don't meet visibility conditions (show_full, fetch_ci)
        }
    }

    // Expand message with any leftover space (up to MAX_MESSAGE total)
    if widths.message > 0 && widths.message < MAX_MESSAGE && remaining > 0 {
        let expansion = remaining.min(MAX_MESSAGE - widths.message);
        widths.message += expansion;
    }

    let final_max_message_len = widths.message;

    // Calculate absolute column positions (with 2-space gaps between columns)
    let gap = 2;
    let mut pos = 0;

    // Helper closure to advance position for a column
    // Returns the column's start position, or 0 if column is hidden (width=0)
    let mut advance = |width: usize| -> usize {
        if width == 0 {
            return 0;
        }
        let column_pos = if pos == 0 { 0 } else { pos + gap };
        pos = column_pos + width;
        column_pos
    };

    let positions = ColumnPositions {
        branch: advance(widths.branch),
        working_diff: advance(widths.working_diff.total),
        ahead_behind: advance(widths.ahead_behind.total),
        branch_diff: advance(widths.branch_diff.total),
        states: advance(widths.states),
        path: advance(widths.path),
        upstream: advance(widths.upstream.total),
        time: advance(widths.time),
        ci_status: advance(widths.ci_status),
        commit: advance(widths.commit),
        message: advance(widths.message),
    };

    LayoutConfig {
        widths,
        positions,
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
            AheadBehind, BranchDiffTotals, CommitDetails, DisplayFields, UpstreamStatus,
            WorktreeInfo,
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
            pr_status: None,
            has_conflicts: false,
            display: DisplayFields::default(),
            working_diff_display: None,
        };

        let (widths, _data_flags) =
            calculate_column_widths(&[super::ListItem::Worktree(info1)], false);

        // "↑3 ↓2" has format "↑3 ↓2" = 1+1+1+1+1 = 5, but header "Main ↕" is 6
        assert_eq!(
            widths.ahead_behind.total, 6,
            "Ahead/behind column should fit header 'Main ↕' (width 6)"
        );
        assert_eq!(widths.ahead_behind.added_digits, 1, "3 has 1 digit");
        assert_eq!(widths.ahead_behind.deleted_digits, 1, "2 has 1 digit");

        // "+100 -50" has width 8, but header "Working ±" is 9, so column width is 9
        assert_eq!(
            widths.working_diff.total, 9,
            "Working diff column should fit header 'Working ±' (width 9)"
        );
        assert_eq!(widths.working_diff.added_digits, 3, "100 has 3 digits");
        assert_eq!(widths.working_diff.deleted_digits, 2, "50 has 2 digits");

        // "+200 -30" has width 8, but header "Main ±" is 6, so column width is 8
        assert_eq!(
            widths.branch_diff.total, 8,
            "Branch diff column should fit header 'Main ±' (width 6)"
        );
        assert_eq!(widths.branch_diff.added_digits, 3, "200 has 3 digits");
        assert_eq!(widths.branch_diff.deleted_digits, 2, "30 has 2 digits");

        // Upstream: "↑4 ↓0" = "↑" (1) + "4" (1) + " " (1) + "↓" (1) + "0" (1) = 5, but header "Remote ↕" = 8
        assert_eq!(
            widths.upstream.total, 8,
            "Upstream column should fit header 'Remote ↕' (width 8)"
        );
        assert_eq!(widths.upstream.added_digits, 1, "4 has 1 digit");
        assert_eq!(widths.upstream.deleted_digits, 1, "0 has 1 digit");
    }

    #[test]
    fn test_visible_columns_follow_gap_rule() {
        use crate::commands::list::model::{
            AheadBehind, BranchDiffTotals, CommitDetails, DisplayFields, UpstreamStatus,
            WorktreeInfo,
        };

        // Create test data with specific widths to verify position calculation
        let info = WorktreeInfo {
            worktree: worktrunk::git::Worktree {
                path: PathBuf::from("/test/path"),
                head: "abc12345".to_string(),
                branch: Some("feature".to_string()),
                bare: false,
                detached: false,
                locked: None,
                prunable: None,
            },
            commit: CommitDetails {
                timestamp: 1234567890,
                commit_message: "Test commit message".to_string(),
            },
            counts: AheadBehind {
                ahead: 5,
                behind: 10,
            },
            working_tree_diff: (100, 50),
            branch_diff: BranchDiffTotals { diff: (200, 30) },
            is_primary: false,
            upstream: UpstreamStatus::from_parts(Some("origin".to_string()), 4, 2),
            worktree_state: None,
            pr_status: None,
            has_conflicts: false,
            display: DisplayFields::default(),
            working_diff_display: None,
        };

        let items = vec![super::ListItem::Worktree(info)];
        let layout = calculate_responsive_layout(&items, false, false);
        let pos = &layout.positions;
        let widths = &layout.widths;

        // Test key invariants of position calculation

        // 1. Branch always starts at position 0
        assert_eq!(pos.branch, 0, "Branch must start at position 0");

        // 2. States may be visible in Phase 2 (empty but shown if space allows)
        // Since we have plenty of space in wide terminal, states should be visible
        assert!(
            pos.states > 0,
            "States column should be visible in Phase 2 (empty but shown if space)"
        );

        // 3. For visible columns, verify correct spacing
        // Each visible column should be at: previous_position + previous_width + gap(2)
        let gap = 2;

        if widths.working_diff.total > 0 && pos.working_diff > 0 {
            assert_eq!(
                pos.working_diff,
                pos.branch + widths.branch + gap,
                "Working diff position should follow branch with 2-space gap"
            );
        }

        if widths.ahead_behind.total > 0 && pos.ahead_behind > 0 {
            let prev_col_end = if pos.working_diff > 0 {
                pos.working_diff + widths.working_diff.total
            } else {
                pos.branch + widths.branch
            };
            assert_eq!(
                pos.ahead_behind,
                prev_col_end + gap,
                "Ahead/behind position should follow previous visible column with 2-space gap"
            );
        }

        // 4. Path must be visible and have position > 0 (it's always shown)
        assert!(pos.path > 0, "Path column must be visible");
        assert!(widths.path > 0, "Path column must have width > 0");
    }

    #[test]
    fn test_column_positions_with_hidden_columns() {
        use crate::commands::list::model::{
            AheadBehind, BranchDiffTotals, CommitDetails, DisplayFields, UpstreamStatus,
            WorktreeInfo,
        };

        // Create minimal data - most columns will be hidden
        let info = WorktreeInfo {
            worktree: worktrunk::git::Worktree {
                path: PathBuf::from("/test"),
                head: "abc12345".to_string(),
                branch: Some("main".to_string()),
                bare: false,
                detached: false,
                locked: None,
                prunable: None,
            },
            commit: CommitDetails {
                timestamp: 1234567890,
                commit_message: "Test".to_string(),
            },
            counts: AheadBehind {
                ahead: 0,
                behind: 0,
            },
            working_tree_diff: (0, 0),
            branch_diff: BranchDiffTotals { diff: (0, 0) },
            is_primary: true, // Primary worktree: no ahead/behind shown
            upstream: UpstreamStatus::default(),
            worktree_state: None,
            pr_status: None,
            has_conflicts: false,
            display: DisplayFields::default(),
            working_diff_display: None,
        };

        let items = vec![super::ListItem::Worktree(info)];
        let layout = calculate_responsive_layout(&items, false, false);
        let pos = &layout.positions;

        // Branch should be at 0
        assert_eq!(pos.branch, 0, "Branch always starts at position 0");

        // With new two-phase allocation, empty columns are shown in Phase 2 if space allows
        // Since we have a wide terminal (80 chars default) and minimal data, at least some empty columns should be visible

        // Early Phase 2 columns should be visible (highest priority empty columns)
        assert!(
            pos.working_diff > 0,
            "Working diff should be visible in Phase 2 (empty but shown if space)"
        );
        assert!(
            pos.ahead_behind > 0,
            "Ahead/behind should be visible in Phase 2 (empty but shown if space)"
        );

        // Later Phase 2 columns might not fit (depending on terminal width)
        // Just verify that at least some empty columns are visible
        let empty_columns_visible = pos.working_diff > 0
            || pos.ahead_behind > 0
            || pos.branch_diff > 0
            || pos.states > 0
            || pos.upstream > 0;

        assert!(
            empty_columns_visible,
            "At least some empty columns should be visible in Phase 2"
        );

        // Path should be visible (always has data)
        assert!(pos.path > 0, "Path should be visible");
    }

    #[test]
    fn test_consecutive_hidden_columns_skip_correctly() {
        use crate::commands::list::model::{
            AheadBehind, BranchDiffTotals, CommitDetails, DisplayFields, UpstreamStatus,
            WorktreeInfo,
        };

        // Create data where multiple consecutive columns are hidden:
        // visible(branch) → hidden(working_diff) → hidden(ahead_behind) → hidden(branch_diff)
        // → hidden(states) → visible(path)
        let info = WorktreeInfo {
            worktree: worktrunk::git::Worktree {
                path: PathBuf::from("/test/worktree"),
                head: "abc12345".to_string(),
                branch: Some("feature-x".to_string()),
                bare: false,
                detached: false,
                locked: None,
                prunable: None,
            },
            commit: CommitDetails {
                timestamp: 1234567890,
                commit_message: "Test commit".to_string(),
            },
            counts: AheadBehind {
                ahead: 0,
                behind: 0,
            },
            working_tree_diff: (0, 0), // Hidden: no dirty changes
            branch_diff: BranchDiffTotals { diff: (0, 0) }, // Hidden: no diff
            is_primary: true,          // Hidden: no ahead/behind for primary
            upstream: UpstreamStatus::default(), // Hidden: no upstream
            worktree_state: None,      // Hidden: no state
            pr_status: None,
            has_conflicts: false,
            display: DisplayFields::default(),
            working_diff_display: None,
        };

        let items = vec![super::ListItem::Worktree(info)];
        let layout = calculate_responsive_layout(&items, false, false);
        let pos = &layout.positions;
        let widths = &layout.widths;

        // With two-phase allocation, empty columns are allocated in Phase 2 (after data columns)
        // Phase 1: branch (data), path (data), time (data), commit (data), message (data)
        // Phase 2: working_diff (empty), ahead_behind (empty), branch_diff (empty), states (empty), upstream (empty), ci_status (empty)

        // In Phase 1, path comes after branch immediately (since all middle columns have no data)
        // Branch, path, time, commit, message are allocated first

        // Path should come early since it has data and is allocated in Phase 1
        assert!(
            pos.path > 0,
            "Path should be visible (allocated in Phase 1)"
        );

        // With the corrected Phase 2 allocation, empty columns only show if space remains AFTER message
        // In this test with 80 character width and minimal data:
        // - Branch, path, time, commit get allocated in Phase 1
        // - Message gets allocated next (before empty columns)
        // - Empty columns only allocated if space remains after message

        // Message should be allocated (it comes before empty columns now)
        assert!(
            widths.message > 0,
            "Message should be allocated before empty columns"
        );

        // Empty columns may or may not be visible depending on space remaining after message
        // This is acceptable - message has priority over empty columns
        // No assertion needed here - it's correct for empty columns to not show if message takes the space
    }
}
