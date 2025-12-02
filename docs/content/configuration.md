+++
title = "Configuration"
weight = 4
+++

Worktrunk uses two configuration files:

- **User config**: `~/.config/worktrunk/config.toml` — Personal settings, LLM commands, saved approvals
- **Project config**: `.config/wt.toml` — Project-specific hooks (checked into version control)

## Project Hooks

Create `.config/wt.toml` in your repository to automate setup and validation at worktree lifecycle events. See the dedicated [Hooks](/hooks/) page for complete documentation.

Quick example:

```toml
[post-create]
install = "npm ci"

[pre-merge]
test = "npm test"
lint = "npm run lint"
```

## LLM Commit Messages

Worktrunk can invoke external commands to generate commit messages. [llm](https://llm.datasette.io/) from Simon Willison is recommended.

### Setup

1. Install llm:
   ```bash
   $ uv tool install -U llm
   ```

2. Configure your API key:
   ```bash
   $ llm install llm-anthropic
   $ llm keys set anthropic
   ```

3. Add to user config (`~/.config/worktrunk/config.toml`):
   ```toml
   [commit-generation]
   command = "llm"
   args = ["-m", "claude-haiku-4-5-20251001"]
   ```

### Usage

`wt merge` generates commit messages automatically, or run `wt step commit` for just the commit step.

For custom prompt templates, see `wt config --help`.

## User Config Reference

Create the user config with defaults:

```bash
$ wt config create
```

This creates `~/.config/worktrunk/config.toml` with documented examples.

### Key settings

```toml
# Worktree path template
# Default: "../{{ main_worktree }}.{{ branch }}"
path-template = "../{{ main_worktree }}.{{ branch }}"

# LLM commit message generation
[commit-generation]
command = "llm"
args = ["-m", "claude-haiku-4-5-20251001"]

# Per-project command approvals (auto-populated)
[approved-commands."my-project"]
"post-create.install" = "npm install"
```

## Shell Integration

Worktrunk needs shell integration to change directories. Install with:

```bash
$ wt config shell install
```

Or manually add to your shell config:

```bash
# bash/zsh
eval "$(wt config shell init bash)"

# fish
wt config shell init fish | source
```

## Environment Variables

Override default behavior with environment variables:

| Variable | Effect |
|----------|--------|
| `WORKTRUNK_CONFIG_PATH` | Override user config location (default: `~/.config/worktrunk/config.toml`) |
| `NO_COLOR` | Disable colored output |
| `CLICOLOR_FORCE` | Force colored output even when not a TTY |

These follow standard conventions — `NO_COLOR` is the [no-color.org](https://no-color.org/) standard.
