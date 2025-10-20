# Worktrunk: Git Worktree Management

A Rust-based CLI tool for managing git worktrees with seamless shell integration.

## Features

- **Shell Integration**: Automatically `cd` to worktrees when switching
- **Multiple Shells**: Supports Bash, Fish, Zsh, Nushell, PowerShell, Elvish, Xonsh, and Oil Shell
- **Customizable**: Configure command prefix and hook behavior
- **Fast**: Built in Rust for performance
- **Clean Design**: Uses the proven "eval init" pattern from tools like zoxide and starship

## Installation

```bash
cargo build --release
# Copy target/release/wt to a directory in your PATH
```

## Setup

One-command setup that includes shell integration and completions:

**Bash** - Add to `~/.bashrc`:
```bash
eval "$(wt init bash)"
```

**Fish** - Add to `~/.config/fish/config.fish`:
```fish
wt init fish | source
```

**Zsh** - Add to `~/.zshrc`:
```bash
eval "$(wt init zsh)"
```

**Nushell** - Add to `~/.config/nushell/env.nu`:
```nu
wt init nushell | save -f ~/.cache/wt-init.nu
```

Then add to `~/.config/nushell/config.nu`:
```nu
source ~/.cache/wt-init.nu
```

**PowerShell** - Add to your PowerShell profile:
```powershell
wt init powershell | Out-String | Invoke-Expression
```

**Elvish** - Add to `~/.config/elvish/rc.elv`:
```elvish
eval (wt init elvish | slurp)
```

**Xonsh** - Add to `~/.xonshrc`:
```python
execx($(wt init xonsh))
```

**Oil Shell** - Add to `~/.config/oil/oshrc`:
```bash
eval "$(wt init oil)"
```

This single command provides:
- Shell integration for automatic `cd` on `wt switch` and `wt remove`
- TAB completion for commands, flags, and branch names (Bash, Fish, Zsh, Oil only)

### What Gets Completed

- **Subcommands**: `wt <TAB>` → shows `list`, `switch`, `remove`, `push`, `merge`
- **Flags**: `wt switch --<TAB>` → shows `--create`, `--base`, `--internal`
- **Branch names**: `wt switch <TAB>` → shows branches without worktrees
- **Target branches**: `wt push <TAB>` → shows all branches

**Notes:**
- Completion is currently supported for Bash, Fish, Zsh, and Oil Shell only. Other shells (Nushell, PowerShell, Elvish, Xonsh) have shell integration but not yet completion.
- Zsh currently uses Bash-compatible completion syntax. Dynamic branch completion may require `bashcompinit`. For best results, use Fish or Bash.
- After updating `wt`, restart your shell or re-run the init command to get new completions
- Debug completion: Set `WT_DEBUG_COMPLETION=1` to see errors
- Performance: Run `cargo bench` to measure completion performance on your system

## Usage

### Basic Commands

```bash
# List all worktrees
wt list

# Switch to a worktree
wt switch feature-branch

# Finish current worktree, returning to primary if current
wt remove

# Push changes between worktrees
wt push target-worktree

# Merge worktree into target branch
wt merge main
```

### Customization

**Custom command prefix:**
```bash
# Use a custom prefix instead of 'wt'
eval "$(wt init bash --cmd myprefix)"

# Now use: myprefix switch, myprefix remove, etc.
```

**Enable prompt hook:**
```bash
# Track worktree changes in your prompt
eval "$(wt init bash --hook prompt)"
```

## How It Works

Worktrunk uses a **directive protocol** to communicate with shell wrappers:

1. Shell wrapper calls `wt switch --internal my-branch`
2. Worktrunk outputs special directives mixed with regular output:
   ```
   __WORKTRUNK_CD__/path/to/worktree
   Switched to worktree: my-branch
   ```
3. Shell wrapper parses output, executes `cd` for directives, displays other lines

This separation keeps the Rust binary focused on git logic while letting the shell handle directory changes.

## Development & Testing

### Running Tests

By default, tests only run for **Tier 1 shells** (bash, fish, zsh) which are easily available:

```bash
cargo test
```

To run tests for **all shells** including Tier 2 shells (nushell, powershell, elvish, xonsh, oil):

```bash
cargo test --features tier-2-integration-tests
```

**Important**: When the `tier-2-integration-tests` feature is enabled, **all tier-2 shells must be installed** or tests will fail. Tests will fail naturally when attempting to execute a missing shell.

**Tier 2 shells** require additional installation:
- **nushell**: Install via [official instructions](https://www.nushell.sh/book/installation.html)
- **powershell**: `apt install powershell` (requires Microsoft repo on Linux)
- **elvish**: `apt install elvish` (Ubuntu 20.04+)
- **xonsh**: `apt install xonsh`
- **oil**: Must be compiled from source (see [Oil Shell docs](https://www.oilshell.org/release/0.24.0/doc/INSTALL.html))

### CI/CD

Two GitHub Actions workflows are provided:

- **`ci.yml`**: Fast feedback with Tier 1 shells only (bash, fish, zsh)
- **`tier-2-integration-tests.yml`**: Comprehensive testing with all shells installed

## Development Status

Current implementation:

- ✅ Shell integration infrastructure (eval init pattern)
- ✅ Template-based shell code generation (Askama)
- ✅ Directive protocol (__WORKTRUNK_CD__)
- ✅ Basic CLI structure
- ⏳ Git primitives (coming next)
- ⏳ Worktree operations (coming next)
- ⏳ Advanced features (push, merge, etc.)

See [TODO.md](TODO.md) for detailed roadmap.

## Architecture

```
wt (Rust binary)
├── Core commands (work standalone)
│   ├── wt list
│   ├── wt push
│   └── wt merge
├── Internal commands (for shell wrapper)
│   ├── wt switch --internal → outputs __WORKTRUNK_CD__ directives
│   ├── wt remove --internal → outputs __WORKTRUNK_CD__ directives
│   └── wt hook prompt → for prompt integration
└── Shell integration
    └── wt init <shell> → outputs shell wrapper function
```

## Design Principles

- **Progressive Enhancement**: Works without shell integration, better with it
- **One Canonical Path**: No options, no configuration unless explicitly needed
- **Fast**: Keep shell integration code minimal (<500ms execution time)
- **Stateless**: Binary doesn't maintain state, shell handles environment

## Inspiration

Worktrunk's shell integration pattern is inspired by successful tools:

- **zoxide**: Smarter cd with frequency tracking
- **starship**: Cross-shell prompt customization
- **direnv**: Per-directory environment variables
- **pyenv**: Python version management with shims

## License

MIT (or your preferred license)
