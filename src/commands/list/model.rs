use rayon::prelude::*;
use std::path::PathBuf;
use worktrunk::git::{GitError, Repository};
use worktrunk::styling::{HINT, HINT_EMOJI, WARNING, WARNING_BOLD, WARNING_EMOJI, println};

use super::ci_status::PrStatus;

/// Display fields shared between WorktreeInfo and BranchInfo
/// These contain formatted strings with ANSI colors for json-pretty output
#[derive(serde::Serialize, Default)]
pub struct DisplayFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commits_display: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch_diff_display: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upstream_display: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ci_status_display: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_display: Option<String>,
}

#[derive(serde::Serialize)]
pub struct WorktreeInfo {
    pub worktree: worktrunk::git::Worktree,
    #[serde(flatten)]
    pub commit: CommitDetails,
    #[serde(flatten)]
    pub counts: AheadBehind,
    pub working_tree_diff: (usize, usize),
    /// Diff between working tree and main branch.
    /// `None` means "not computed" (optimization: skipped when trees differ).
    /// `Some((0, 0))` means working tree matches main exactly.
    /// `Some((a, d))` means a lines added, d deleted vs main.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_tree_diff_with_main: Option<(usize, usize)>,
    #[serde(flatten)]
    pub branch_diff: BranchDiffTotals,
    pub is_primary: bool,
    #[serde(flatten)]
    pub upstream: UpstreamStatus,
    pub worktree_state: Option<String>,
    pub pr_status: Option<PrStatus>,
    pub has_conflicts: bool,
    /// Git status symbols (=, ↑, ↓, ⇡, ⇣, ?, !, +, », ✘) indicating working tree state
    pub status_symbols: StatusSymbols,
    /// User-defined status from worktrunk.status git config
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_status: Option<String>,

    // Display fields for json-pretty format (with ANSI colors)
    #[serde(flatten)]
    pub display: DisplayFields,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_diff_display: Option<String>,
}

#[derive(serde::Serialize)]
pub struct BranchInfo {
    pub name: String,
    pub head: String,
    #[serde(flatten)]
    pub commit: CommitDetails,
    #[serde(flatten)]
    pub counts: AheadBehind,
    #[serde(flatten)]
    pub branch_diff: BranchDiffTotals,
    #[serde(flatten)]
    pub upstream: UpstreamStatus,
    pub pr_status: Option<PrStatus>,
    pub has_conflicts: bool,
    /// User-defined status from `worktrunk.status.<branch>` git config
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_status: Option<String>,

    // Display fields for json-pretty format (with ANSI colors)
    #[serde(flatten)]
    pub display: DisplayFields,
}

#[derive(serde::Serialize, Clone)]
pub struct CommitDetails {
    pub timestamp: i64,
    pub commit_message: String,
}

impl CommitDetails {
    fn gather(repo: &Repository, head: &str) -> Result<Self, GitError> {
        Ok(Self {
            timestamp: repo.commit_timestamp(head)?,
            commit_message: repo.commit_message(head)?,
        })
    }
}

#[derive(serde::Serialize, Default, Clone)]
pub struct AheadBehind {
    pub ahead: usize,
    pub behind: usize,
}

impl AheadBehind {
    fn compute(repo: &Repository, base: Option<&str>, head: &str) -> Result<Self, GitError> {
        let Some(base) = base else {
            return Ok(Self::default());
        };

        let (ahead, behind) = repo.ahead_behind(base, head)?;
        Ok(Self { ahead, behind })
    }
}

#[derive(serde::Serialize, Default, Clone)]
pub struct BranchDiffTotals {
    #[serde(rename = "branch_diff")]
    pub diff: (usize, usize),
}

impl BranchDiffTotals {
    fn compute(repo: &Repository, base: Option<&str>, head: &str) -> Result<Self, GitError> {
        let Some(base) = base else {
            return Ok(Self::default());
        };

        let diff = repo.branch_diff_stats(base, head)?;
        Ok(Self { diff })
    }
}

#[derive(serde::Serialize, Default, Clone)]
pub struct UpstreamStatus {
    #[serde(rename = "upstream_remote")]
    remote: Option<String>,
    #[serde(rename = "upstream_ahead")]
    ahead: usize,
    #[serde(rename = "upstream_behind")]
    behind: usize,
}

impl UpstreamStatus {
    fn calculate(repo: &Repository, branch: Option<&str>, head: &str) -> Result<Self, GitError> {
        let Some(branch) = branch else {
            return Ok(Self::default());
        };

        match repo.upstream_branch(branch) {
            Ok(Some(upstream_branch)) => {
                let remote = upstream_branch
                    .split_once('/')
                    .map(|(remote, _)| remote)
                    .unwrap_or("origin")
                    .to_string();
                let (ahead, behind) = repo.ahead_behind(&upstream_branch, head)?;
                Ok(Self {
                    remote: Some(remote),
                    ahead,
                    behind,
                })
            }
            _ => Ok(Self::default()),
        }
    }

    pub fn active(&self) -> Option<(&str, usize, usize)> {
        self.remote
            .as_deref()
            .map(|remote| (remote, self.ahead, self.behind))
    }

    #[cfg(test)]
    pub fn from_parts(remote: Option<String>, ahead: usize, behind: usize) -> Self {
        Self {
            remote,
            ahead,
            behind,
        }
    }
}

/// Unified type for displaying worktrees and branches in the same table
#[derive(serde::Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
#[allow(clippy::large_enum_variant)]
pub enum ListItem {
    Worktree(WorktreeInfo),
    Branch(BranchInfo),
}

pub struct ListData {
    pub items: Vec<ListItem>,
    pub current_worktree_path: Option<PathBuf>,
}

impl ListItem {
    pub fn branch_name(&self) -> &str {
        match self {
            ListItem::Worktree(wt) => wt.worktree.branch.as_deref().unwrap_or("(detached)"),
            ListItem::Branch(br) => &br.name,
        }
    }

    pub fn is_primary(&self) -> bool {
        matches!(self, ListItem::Worktree(wt) if wt.is_primary)
    }

    pub fn commit_timestamp(&self) -> i64 {
        match self {
            ListItem::Worktree(info) => info.commit.timestamp,
            ListItem::Branch(info) => info.commit.timestamp,
        }
    }

    pub fn head(&self) -> &str {
        match self {
            ListItem::Worktree(info) => &info.worktree.head,
            ListItem::Branch(info) => &info.head,
        }
    }

    pub fn commit_details(&self) -> &CommitDetails {
        match self {
            ListItem::Worktree(info) => &info.commit,
            ListItem::Branch(info) => &info.commit,
        }
    }

    pub fn counts(&self) -> &AheadBehind {
        match self {
            ListItem::Worktree(info) => &info.counts,
            ListItem::Branch(info) => &info.counts,
        }
    }

    pub fn branch_diff(&self) -> &BranchDiffTotals {
        match self {
            ListItem::Worktree(info) => &info.branch_diff,
            ListItem::Branch(info) => &info.branch_diff,
        }
    }

    pub fn upstream(&self) -> &UpstreamStatus {
        match self {
            ListItem::Worktree(info) => &info.upstream,
            ListItem::Branch(info) => &info.upstream,
        }
    }

    pub fn worktree_info(&self) -> Option<&WorktreeInfo> {
        match self {
            ListItem::Worktree(info) => Some(info),
            ListItem::Branch(_) => None,
        }
    }

    pub fn worktree_path(&self) -> Option<&PathBuf> {
        self.worktree_info().map(|info| &info.worktree.path)
    }

    pub fn pr_status(&self) -> Option<&PrStatus> {
        match self {
            ListItem::Worktree(info) => info.pr_status.as_ref(),
            ListItem::Branch(info) => info.pr_status.as_ref(),
        }
    }

    /// Get combined status (git symbols + user status)
    /// For worktrees: uses WorktreeInfo.combined_status()
    /// For branches: returns user status (branches have no git status symbols)
    pub fn combined_status(&self) -> String {
        match self {
            ListItem::Worktree(info) => info.combined_status(),
            ListItem::Branch(info) => {
                // Branch-only entries show just the user status (no git symbols)
                // If no user status, show "·" to indicate "branch without worktree"
                info.user_status.clone().unwrap_or_else(|| "·".to_string())
            }
        }
    }
}

impl BranchInfo {
    /// Create BranchInfo from a branch name, enriching it with git metadata
    fn from_branch(
        branch: &str,
        repo: &Repository,
        primary_branch: Option<&str>,
        fetch_ci: bool,
        check_conflicts: bool,
    ) -> Result<Self, GitError> {
        // Get the commit SHA for this branch
        let head = repo.run_command(&["rev-parse", branch])?.trim().to_string();

        let commit = CommitDetails::gather(repo, &head)?;
        let counts = AheadBehind::compute(repo, primary_branch, &head)?;
        let branch_diff = BranchDiffTotals::compute(repo, primary_branch, &head)?;
        let upstream = UpstreamStatus::calculate(repo, Some(branch), &head)?;

        let pr_status = if fetch_ci {
            PrStatus::detect(branch, &head)
        } else {
            None
        };

        let has_conflicts = if check_conflicts {
            if let Some(base) = primary_branch {
                repo.has_merge_conflicts(base, &head)?
            } else {
                false
            }
        } else {
            false
        };

        // Read user-defined status from git config (branch-keyed only, no worktree)
        let user_status = read_branch_keyed_status(repo, branch);

        // Create display fields with status
        // For branches without worktrees, status is just user status or "·"
        let status_display = Some(user_status.clone().unwrap_or_else(|| "·".to_string()));

        let display = DisplayFields {
            status_display,
            ..Default::default()
        };

        Ok(BranchInfo {
            name: branch.to_string(),
            head,
            commit,
            counts,
            branch_diff,
            upstream,
            pr_status,
            has_conflicts,
            user_status,
            display,
        })
    }
}

/// Read user-defined status from branch-keyed config only (`worktrunk.status.<branch>`)
/// Used for branch-only entries that don't have a worktree
fn read_branch_keyed_status(repo: &Repository, branch: &str) -> Option<String> {
    let config_key = format!("worktrunk.status.{}", branch);
    repo.run_command(&["config", "--get", &config_key])
        .ok()
        .map(|output| output.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Read user-defined status from git config
/// Tries worktree-specific config first (`worktrunk.status`), then falls back to branch-keyed (`worktrunk.status.<branch>`)
/// Used for worktree entries
fn read_user_status(repo: &Repository, branch: Option<&str>) -> Option<String> {
    // Try worktree-specific config first (requires extensions.worktreeConfig)
    if let Ok(output) = repo.run_command(&["config", "--worktree", "--get", "worktrunk.status"]) {
        let status = output.trim().to_string();
        if !status.is_empty() {
            return Some(status);
        }
    }

    // Fall back to branch-keyed config (works everywhere)
    let branch = branch?;
    read_branch_keyed_status(repo, branch)
}

/// Main branch divergence state
///
/// Represents relationship to the main/primary branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MainDivergence {
    /// Up to date with main branch
    #[default]
    None,
    /// Ahead of main (has commits main doesn't have)
    Ahead,
    /// Behind main (missing commits from main)
    Behind,
    /// Diverged (both ahead and behind main)
    Diverged,
}

impl std::fmt::Display for MainDivergence {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::None => Ok(()),
            Self::Ahead => write!(f, "↑"),
            Self::Behind => write!(f, "↓"),
            Self::Diverged => write!(f, "↕"),
        }
    }
}

impl serde::Serialize for MainDivergence {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Serialize as empty string for None, or the character for other variants
        serializer.serialize_str(&self.to_string())
    }
}

/// Upstream/remote divergence state
///
/// Represents relationship to the remote tracking branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UpstreamDivergence {
    /// Up to date with remote
    #[default]
    None,
    /// Ahead of remote (has commits remote doesn't have)
    Ahead,
    /// Behind remote (missing commits from remote)
    Behind,
    /// Diverged (both ahead and behind remote)
    Diverged,
}

impl std::fmt::Display for UpstreamDivergence {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::None => Ok(()),
            Self::Ahead => write!(f, "⇡"),
            Self::Behind => write!(f, "⇣"),
            Self::Diverged => write!(f, "⇅"),
        }
    }
}

impl serde::Serialize for UpstreamDivergence {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Serialize as empty string for None, or the character for other variants
        serializer.serialize_str(&self.to_string())
    }
}

/// Branch state relative to main/primary branch
///
/// Represents whether the branch matches main or has no commits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BranchState {
    /// Normal branch state (neither matches main nor empty)
    #[default]
    None,
    /// Working tree identical to main branch
    MatchesMain,
    /// No commits ahead and clean working tree (not matching main)
    NoCommits,
}

impl std::fmt::Display for BranchState {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::None => Ok(()),
            Self::MatchesMain => write!(f, "≡"),
            Self::NoCommits => write!(f, "∅"),
        }
    }
}

impl serde::Serialize for BranchState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// Git operation in progress
///
/// Represents active rebase or merge operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GitOperation {
    /// No git operation in progress
    #[default]
    None,
    /// Rebase in progress
    Rebase,
    /// Merge in progress
    Merge,
}

impl std::fmt::Display for GitOperation {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::None => Ok(()),
            Self::Rebase => write!(f, "↻"),
            Self::Merge => write!(f, "⋈"),
        }
    }
}

impl serde::Serialize for GitOperation {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// Structured status symbols for aligned rendering
///
/// Symbols are categorized to enable vertical alignment in table output:
/// - Position 0a: Conflicts (=)
/// - Position 0b: Branch state (≡, ∅)
/// - Position 0c: Git operation (↻, ⋈)
/// - Position 0d: Worktree attributes (◇, ⊠, ⚠)
/// - Position 1: Main branch divergence (↑, ↓, ↕)
/// - Position 2: Remote/upstream divergence (⇡, ⇣, ⇅)
/// - Position 3+: Working tree symbols (?, !, +, », ✘)
///
/// ## Mutual Exclusivity
///
/// **Mutually exclusive (enforced by type system):**
/// - ≡ vs ∅: Branch state (BranchState enum)
/// - ↻ vs ⋈: Git operation (GitOperation enum)
/// - ↑ vs ↓ vs ↕: Main divergence (MainDivergence enum)
/// - ⇡ vs ⇣ vs ⇅: Upstream divergence (UpstreamDivergence enum)
///
/// **NOT mutually exclusive (can co-occur):**
/// - = can occur with any other symbol
/// - ◇, ⊠, ⚠: Worktree can be bare+locked, bare+prunable, etc.
/// - All working tree symbols (?!+»✘): Can have multiple types of changes
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct StatusSymbols {
    /// Merge conflicts indicator
    /// Position 0a - Boolean flag (= or empty)
    has_conflicts: bool,

    /// Branch state relative to main
    /// Position 0b - MUTUALLY EXCLUSIVE (enforced by enum)
    branch_state: BranchState,

    /// Git operation in progress
    /// Position 0c - MUTUALLY EXCLUSIVE (enforced by enum)
    git_operation: GitOperation,

    /// Worktree attributes: ◇, ⊠, ⚠
    /// Position 0d - NOT mutually exclusive (can combine like "◇⊠")
    worktree_attrs: String,

    /// Main branch divergence state
    /// Position 1 - MUTUALLY EXCLUSIVE (enforced by enum)
    main_divergence: MainDivergence,

    /// Remote/upstream divergence state
    /// Position 2 - MUTUALLY EXCLUSIVE (enforced by enum)
    upstream_divergence: UpstreamDivergence,

    /// Working tree changes: ?, !, +, », ✘
    /// Position 3+ - NOT mutually exclusive (can have "?!+" etc.)
    working_tree: String,
}

impl StatusSymbols {
    /// Render symbols with full alignment
    ///
    /// Aligns all symbol types at fixed positions:
    /// - Position 0a: Conflicts (= or space)
    /// - Position 0b: Branch state (≡, ∅, or space)
    /// - Position 0c: Git operation (↻, ⋈, or space)
    /// - Position 0d: Worktree attributes (◇⊠⚠ or space)
    /// - Position 1: Main divergence (↑, ↓, ↕, or space)
    /// - Position 2: Upstream divergence (⇡, ⇣, ⇅, or space)
    /// - Position 3+: Working tree symbols (?, !, +, », ✘)
    ///
    /// This ensures vertical scannability - each symbol type appears at the same
    /// column position across all rows.
    pub fn render(&self) -> String {
        let mut result = String::with_capacity(12);

        if self.is_empty() {
            return result;
        }

        // Check if we have any symbols after each position (for conditional spacing)
        let has_post_0a = self.branch_state != BranchState::None
            || self.git_operation != GitOperation::None
            || !self.worktree_attrs.is_empty()
            || self.main_divergence != MainDivergence::None
            || self.upstream_divergence != UpstreamDivergence::None
            || !self.working_tree.is_empty();

        let has_post_0b = self.git_operation != GitOperation::None
            || !self.worktree_attrs.is_empty()
            || self.main_divergence != MainDivergence::None
            || self.upstream_divergence != UpstreamDivergence::None
            || !self.working_tree.is_empty();

        let has_post_0c = !self.worktree_attrs.is_empty()
            || self.main_divergence != MainDivergence::None
            || self.upstream_divergence != UpstreamDivergence::None
            || !self.working_tree.is_empty();

        let has_post_0d = self.main_divergence != MainDivergence::None
            || self.upstream_divergence != UpstreamDivergence::None
            || !self.working_tree.is_empty();

        // Position 0a: Conflicts
        if self.has_conflicts {
            result.push('=');
        } else if has_post_0a {
            result.push(' ');
        }

        // Position 0b: Branch state (≡ or ∅)
        let branch_state_str = self.branch_state.to_string();
        if !branch_state_str.is_empty() {
            result.push_str(&branch_state_str);
        } else if has_post_0b {
            result.push(' ');
        }

        // Position 0c: Git operation (↻ or ⋈)
        let git_op_str = self.git_operation.to_string();
        if !git_op_str.is_empty() {
            result.push_str(&git_op_str);
        } else if has_post_0c {
            result.push(' ');
        }

        // Position 0d: Worktree attributes (◇⊠⚠)
        if !self.worktree_attrs.is_empty() {
            result.push_str(&self.worktree_attrs);
        } else if has_post_0d {
            result.push(' ');
        }

        // Position 1: Main divergence (↑, ↓, or ↕)
        let main_str = self.main_divergence.to_string();
        if !main_str.is_empty() {
            result.push_str(&main_str);
        } else {
            // Only add space if we have upstream or working_tree symbols
            if self.upstream_divergence != UpstreamDivergence::None || !self.working_tree.is_empty()
            {
                result.push(' ');
            }
        }

        // Position 2: Upstream divergence (⇡, ⇣, or ⇅)
        let upstream_str = self.upstream_divergence.to_string();
        if !upstream_str.is_empty() {
            result.push_str(&upstream_str);
        } else {
            // Only add space if we have working_tree symbols
            if !self.working_tree.is_empty() {
                result.push(' ');
            }
        }

        // Position 3+: Working tree symbols
        result.push_str(&self.working_tree);

        result
    }

    /// Check if symbols are empty
    pub fn is_empty(&self) -> bool {
        !self.has_conflicts
            && self.branch_state == BranchState::None
            && self.git_operation == GitOperation::None
            && self.worktree_attrs.is_empty()
            && self.main_divergence == MainDivergence::None
            && self.upstream_divergence == UpstreamDivergence::None
            && self.working_tree.is_empty()
    }
}

/// Git status information parsed from `git status --porcelain`
struct GitStatusInfo {
    /// Whether the working tree has any changes (staged or unstaged)
    is_dirty: bool,
    /// Status symbols (structured for alignment)
    symbols: StatusSymbols,
}

/// Parse git status --porcelain output to determine dirty state and status symbols
/// This combines the dirty check and symbol computation in a single git command
fn parse_git_status(
    repo: &Repository,
    main_ahead: usize,
    main_behind: usize,
    upstream_ahead: usize,
    upstream_behind: usize,
) -> Result<GitStatusInfo, GitError> {
    let status_output = repo.run_command(&["status", "--porcelain"])?;

    let mut has_conflicts = false;
    let mut has_untracked = false;
    let mut has_modified = false;
    let mut has_staged = false;
    let mut has_renamed = false;
    let mut has_deleted = false;
    let mut is_dirty = false;

    for line in status_output.lines() {
        if line.len() < 2 {
            continue;
        }

        is_dirty = true; // Any line means changes exist

        // Get status codes (first two bytes for ASCII compatibility)
        let bytes = line.as_bytes();
        let index_status = bytes[0] as char;
        let worktree_status = bytes[1] as char;

        // Unmerged paths (actual conflicts in working tree)
        // U = unmerged, D = both deleted, A = both added
        if index_status == 'U'
            || worktree_status == 'U'
            || (index_status == 'D' && worktree_status == 'D')
            || (index_status == 'A' && worktree_status == 'A')
        {
            has_conflicts = true;
        }

        // Untracked files
        if index_status == '?' && worktree_status == '?' {
            has_untracked = true;
        }

        // Modified (unstaged changes in working tree)
        if worktree_status == 'M' {
            has_modified = true;
        }

        // Staged files (changes in index)
        // Includes: A (added), M (modified), C (copied), but excludes D/R
        if index_status == 'A' || index_status == 'M' || index_status == 'C' {
            has_staged = true;
        }

        // Renamed files (staged rename)
        if index_status == 'R' {
            has_renamed = true;
        }

        // Deleted files (staged or unstaged)
        if index_status == 'D' || worktree_status == 'D' {
            has_deleted = true;
        }
    }

    // Build working tree string
    let mut working_tree = String::new();
    if has_untracked {
        working_tree.push('?');
    }
    if has_modified {
        working_tree.push('!');
    }
    if has_staged {
        working_tree.push('+');
    }
    if has_renamed {
        working_tree.push('»');
    }
    if has_deleted {
        working_tree.push('✘');
    }

    // Build structured symbols for aligned rendering
    let symbols = StatusSymbols {
        has_conflicts,
        main_divergence: match (main_ahead > 0, main_behind > 0) {
            (true, true) => MainDivergence::Diverged, // Both ahead and behind
            (true, false) => MainDivergence::Ahead,   // Ahead only
            (false, true) => MainDivergence::Behind,  // Behind only
            (false, false) => MainDivergence::None,   // Up to date
        },
        upstream_divergence: match (upstream_ahead > 0, upstream_behind > 0) {
            (true, true) => UpstreamDivergence::Diverged, // Both ahead and behind
            (true, false) => UpstreamDivergence::Ahead,   // Ahead only
            (false, true) => UpstreamDivergence::Behind,  // Behind only
            (false, false) => UpstreamDivergence::None,   // Up to date
        },
        working_tree,
        ..Default::default()
    };

    Ok(GitStatusInfo { is_dirty, symbols })
}

impl WorktreeInfo {
    /// Create WorktreeInfo from a Worktree, enriching it with git metadata
    fn from_worktree(
        wt: &worktrunk::git::Worktree,
        primary: &worktrunk::git::Worktree,
        fetch_ci: bool,
        check_conflicts: bool,
    ) -> Result<Self, GitError> {
        let wt_repo = Repository::at(&wt.path);
        let is_primary = wt.path == primary.path;

        let commit = CommitDetails::gather(&wt_repo, &wt.head)?;
        let base_branch = primary.branch.as_deref().filter(|_| !is_primary);
        let counts = AheadBehind::compute(&wt_repo, base_branch, &wt.head)?;
        let upstream = UpstreamStatus::calculate(&wt_repo, wt.branch.as_deref(), &wt.head)?;

        // Parse git status once for both dirty check and status symbols
        // Pass both main and upstream ahead/behind counts
        let (upstream_ahead, upstream_behind) = upstream
            .active()
            .map(|(_, ahead, behind)| (ahead, behind))
            .unwrap_or((0, 0));
        let status_info = parse_git_status(
            &wt_repo,
            counts.ahead,
            counts.behind,
            upstream_ahead,
            upstream_behind,
        )?;

        let working_tree_diff = if status_info.is_dirty {
            wt_repo.working_tree_diff_stats()?
        } else {
            (0, 0) // Clean working tree
        };

        // Use tree equality check instead of expensive diff for "matches main"
        let working_tree_diff_with_main = if let Some(base) = base_branch {
            // Get tree hashes for HEAD and base branch
            let head_tree = wt_repo
                .run_command(&["rev-parse", "HEAD^{tree}"])?
                .trim()
                .to_string();
            let base_tree = wt_repo
                .run_command(&["rev-parse", &format!("{}^{{tree}}", base)])?
                .trim()
                .to_string();

            if head_tree == base_tree {
                // Trees are identical - check if working tree is also clean
                if status_info.is_dirty {
                    // Rare case: trees match but working tree has uncommitted changes
                    // Need to compute actual diff to get accurate line counts
                    Some(wt_repo.working_tree_diff_vs_ref(base)?)
                } else {
                    // Trees match and working tree is clean → matches main exactly
                    Some((0, 0))
                }
            } else {
                // Trees differ - skip the expensive scan
                // Return None to indicate "not computed" (optimization)
                None
            }
        } else {
            Some((0, 0)) // Primary worktree always matches itself
        };
        let branch_diff = BranchDiffTotals::compute(&wt_repo, base_branch, &wt.head)?;

        // Get worktree state (merge/rebase/etc)
        let worktree_state = wt_repo.worktree_state()?;

        let pr_status = if fetch_ci {
            wt.branch
                .as_deref()
                .and_then(|branch| PrStatus::detect(branch, &wt.head))
        } else {
            None
        };

        let has_conflicts = if check_conflicts {
            if let Some(base) = base_branch {
                wt_repo.has_merge_conflicts(base, &wt.head)?
            } else {
                false
            }
        } else {
            false
        };

        // Build complete status symbols with type-safe enums
        // Order: = ≡∅ ↻⋈ ◇⊠⚠ | ↑↓ | ⇡⇣ | ?!+»✘
        //        ^0a 0b 0c 0d^  ^1^  ^2^  ^3+^
        let mut symbols = status_info.symbols;

        // Add merge conflicts indicator if this branch has conflicts with base
        // (different from git status conflicts which are already in has_conflicts)
        if has_conflicts {
            symbols.has_conflicts = true;
        }

        // Branch state: ≡ matches main, ∅ no commits (mutually exclusive)
        if !is_primary {
            if working_tree_diff_with_main == Some((0, 0)) {
                symbols.branch_state = BranchState::MatchesMain;
            } else if counts.ahead == 0 && working_tree_diff == (0, 0) {
                symbols.branch_state = BranchState::NoCommits;
            }
        }

        // Git operation: ↻ rebase, ⋈ merge (mutually exclusive)
        if let Some(state) = &worktree_state {
            if state.contains("rebase") {
                symbols.git_operation = GitOperation::Rebase;
            } else if state.contains("merge") {
                symbols.git_operation = GitOperation::Merge;
            }
        }

        // Worktree attributes: ◇ bare, ⊠ locked, ⚠ prunable (can combine)
        if wt.bare {
            symbols.worktree_attrs.push('◇');
        }
        if wt.locked.is_some() {
            symbols.worktree_attrs.push('⊠');
        }
        if wt.prunable.is_some() {
            symbols.worktree_attrs.push('⚠');
        }

        // Read user-defined status from git config (worktree-specific or branch-keyed)
        let user_status = read_user_status(&wt_repo, wt.branch.as_deref());

        // Create display fields with rendered status
        let status_display = if !symbols.is_empty() || user_status.is_some() {
            let mut rendered = symbols.render();
            if let Some(ref user) = user_status {
                rendered.push_str(user);
            }
            Some(rendered)
        } else {
            None
        };

        let display = DisplayFields {
            status_display,
            ..Default::default()
        };

        Ok(WorktreeInfo {
            worktree: wt.clone(),
            commit,
            counts,
            working_tree_diff,
            working_tree_diff_with_main,
            branch_diff,
            is_primary,
            upstream,
            worktree_state,
            pr_status,
            has_conflicts,
            status_symbols: symbols,
            user_status,
            display,
            working_diff_display: None,
        })
    }

    /// Combine git status symbols and user-defined status
    /// Returns the combined string with aligned rendering
    pub fn combined_status(&self) -> String {
        if !self.status_symbols.is_empty() {
            let rendered = self.status_symbols.render();
            if let Some(ref user_status) = self.user_status {
                format!("{}{}", rendered, user_status)
            } else {
                rendered
            }
        } else if let Some(ref user_status) = self.user_status {
            user_status.clone()
        } else {
            String::new()
        }
    }
}

/// Gather list data (worktrees + optional branches).
pub fn gather_list_data(
    repo: &Repository,
    show_branches: bool,
    fetch_ci: bool,
    check_conflicts: bool,
) -> Result<Option<ListData>, GitError> {
    let worktrees = repo.list_worktrees()?;

    if worktrees.worktrees.is_empty() {
        return Ok(None);
    }

    // Get primary worktree - clone it for use in closure
    let primary = worktrees.worktrees[0].clone();

    // Get current worktree to identify active one
    let current_worktree_path = repo.worktree_root().ok();

    // Gather enhanced information for all worktrees in parallel
    let worktree_results: Vec<Result<WorktreeInfo, GitError>> = worktrees
        .worktrees
        .par_iter()
        .map(|wt| WorktreeInfo::from_worktree(wt, &primary, fetch_ci, check_conflicts))
        .collect();

    // Build list of items to display (worktrees + optional branches)
    let mut items: Vec<ListItem> = Vec::new();

    // Process worktree results
    for result in worktree_results {
        match result {
            Ok(info) => items.push(ListItem::Worktree(info)),
            Err(e) => {
                // Worktree enrichment failures are critical - propagate error
                return Err(e);
            }
        }
    }

    // Process branches in parallel if requested
    if show_branches {
        let available_branches = repo.available_branches()?;
        let primary_branch = primary.branch.as_deref();

        let branch_results: Vec<(String, Result<BranchInfo, GitError>)> = available_branches
            .par_iter()
            .map(|branch| {
                let result = BranchInfo::from_branch(
                    branch,
                    repo,
                    primary_branch,
                    fetch_ci,
                    check_conflicts,
                );
                (branch.clone(), result)
            })
            .collect();

        for (branch, result) in branch_results {
            match result {
                Ok(branch_info) => items.push(ListItem::Branch(branch_info)),
                Err(e) => {
                    println!(
                        "{WARNING_EMOJI} {WARNING}Failed to enrich branch {WARNING_BOLD}{branch}{WARNING_BOLD:#}: {e}{WARNING:#}"
                    );
                    println!(
                        "{HINT_EMOJI} {HINT}This branch will be shown with limited information{HINT:#}"
                    );
                }
            }
        }
    }

    // Sort by:
    // 1. Main worktree (primary) always first
    // 2. Current worktree second (if not main)
    // 3. Remaining worktrees by age (most recent first)
    items.sort_by_key(|item| {
        let is_primary = item.is_primary();
        let is_current = item
            .worktree_path()
            .and_then(|p| current_worktree_path.as_ref().map(|cp| p == cp))
            .unwrap_or(false);

        // Primary sort key: 0 = main, 1 = current (non-main), 2 = others
        let priority = if is_primary {
            0
        } else if is_current {
            1
        } else {
            2
        };

        // Secondary sort: timestamp (reversed for descending order)
        (priority, std::cmp::Reverse(item.commit_timestamp()))
    });

    Ok(Some(ListData {
        items,
        current_worktree_path,
    }))
}
