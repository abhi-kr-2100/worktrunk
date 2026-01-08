# WorktreeResolutionMode Implementation

## Overview

Implemented `WorktreeResolutionMode` enum to control branch name resolution behavior in worktrunk. This allows fuzzy matching only for switching to existing worktrees while enforcing strict matching for creating branches.

## Changes

### 1. New Type: `WorktreeResolutionMode`

**File:** `src/git/repository/mod.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorktreeResolutionMode {
    /// Exact match only - branch name must exist exactly
    Strict,
    /// Allow fuzzy matching on branch names
    Fuzzy,
}
```

### 2. Updated Methods

#### `Repository::resolve_worktree_name()`
- **Before:** `fn resolve_worktree_name(&self, name: &str) -> Result<String>`
- **After:** `fn resolve_worktree_name(&self, name: &str, mode: WorktreeResolutionMode) -> Result<String>`
- Fuzzy matching now only occurs when `mode == WorktreeResolutionMode::Fuzzy`

#### `Repository::resolve_target_branch()`
- **Before:** `fn resolve_target_branch(&self, target: Option<&str>) -> Result<String>`
- **After:** `fn resolve_target_branch(&self, target: Option<&str>, mode: WorktreeResolutionMode) -> Result<String>`
- Passes resolution mode through to `resolve_worktree_name()`

### 3. Call Sites Updated

#### `src/commands/worktree.rs` - `handle_switch()`
```rust
// Use fuzzy resolution only when switching to existing worktree (no --create)
let resolution_mode = if create {
    WorktreeResolutionMode::Strict
} else {
    WorktreeResolutionMode::Fuzzy
};

let resolved_branch = repo
    .resolve_worktree_name(branch, resolution_mode)?;

// Explicit base args always use strict
let resolved_base = if let Some(base_str) = base {
    Some(repo.resolve_worktree_name(base_str, WorktreeResolutionMode::Strict)?)
} else {
    None
};
```

#### All Other Commands
- `src/commands/merge.rs` - `handle_merge()`
- `src/commands/step_commands.rs` - `step_commit()`, `handle_squash()`, `handle_rebase()`
- `src/commands/worktree.rs` - `handle_push()`, branch resolution for base

All use `WorktreeResolutionMode::Strict` since they operate on existing branches that must be exact matches.

### 4. Exports

**File:** `src/git/mod.rs`

Added `WorktreeResolutionMode` to public API:
```rust
pub use repository::{Repository, ResolvedWorktree, WorktreeResolutionMode, set_base_path};
```

## Behavior

### Before
- `wt switch feat` with "feature/long-branch-name" existing → fuzzy matches → switches
- `wt switch -c feat` with "feature/long-branch-name" existing → fuzzy matches → ERROR (branch exists)
- Inconsistent behavior across operations

### After
- `wt switch feat` (no `--create`) → **Fuzzy mode** → matches "feature/long-branch-name" ✓
- `wt switch -c feat` (with `--create`) → **Strict mode** → creates new "feat" branch ✓
- All merge/rebase/push operations → **Strict mode** → requires exact match

## Testing

All tests pass:
- ✓ Unit tests: 399 passed
- ✓ Integration tests: 720+ passed (697 + merge/switch specific tests)
  - `test_switch_create*` tests: 6 passed
  - `test_switch_existing*` tests: 4 passed
  - Merge/rebase tests: 101+ passed

## Verification

Quick test demonstrating the behavior:

```bash
# Create branch "feature/long-branch-name"
git checkout -b "feature/long-branch-name"

# Switch with fuzzy match (no --create)
wt switch "feat"  # Matches "feature/long-branch-name" via fuzzy matching

# Switch with strict match (--create)
wt switch -c "feat"  # Creates new exact branch "feat" (strict mode)
```

## Design Rationale

1. **Fuzzy matching for switching only** — Users frequently abbreviate existing branch names when switching (muscle memory, convenience)
2. **Strict matching for creation** — Prevents accidental branch creation with misspelled names
3. **Strict matching for merge/rebase targets** — These are explicit operations that should target exact branches
4. **Explicit base arguments always strict** — Base branches are explicit arguments and should require exact matches
