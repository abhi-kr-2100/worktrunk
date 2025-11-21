#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
DEMO_DIR="$SCRIPT_DIR/wt-demo"
OUT_DIR="$DEMO_DIR/out"
DEMO_HOME="${DEMO_HOME:-$DEMO_ROOT}"
LOG="$OUT_DIR/record.log"
TAPE_TEMPLATE="$DEMO_DIR/demo.tape"
TAPE_RENDERED="$OUT_DIR/.rendered.tape"
STARSHIP_CONFIG_PATH="$OUT_DIR/starship.toml"
OUTPUT_GIF="$OUT_DIR/wt-demo.gif"
BARE_REMOTE=""
DEMO_REPO=""
DEMO_WORK_BASE=""

cleanup() {
  rm -f "$TAPE_RENDERED"
}

require_bin() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Missing dependency: $1" >&2
    exit 1
  fi
}

write_starship_config() {
  mkdir -p "$(dirname "$STARSHIP_CONFIG_PATH")"
  cat >"$STARSHIP_CONFIG_PATH" <<'CFG'
format = "$directory$character"
palette = "gh_light"

[palettes.gh_light]
fg = "#1f2328"
bg = "#ffffff"
blue = "#0969da"
yellow = "#d29922"
green = "#2ea043"
red = "#d73a49"
muted = "#57606a"

[directory]
style = "bold fg:blue"
truncation_length = 3
truncate_to_repo = true
home_symbol = "~"

[git_branch]
style = "fg:muted"
symbol = " "
format = " [$symbol$branch]($style)"

[git_status]
style = "fg:red"
format = " [$all_status$ahead_behind]($style)"
conflicted = "⇕"
ahead = "⇡"
behind = "⇣"
staged = "+"
modified = "!"
untracked = "?"

[cmd_duration]
min_time = 500
# Keep duration but drop the timer icon to reduce prompt noise.
format = " [$duration]($style)"
style = "fg:muted"

[character]
success_symbol = "[❯](fg:green)"
error_symbol = "[❯](fg:red)"
vicmd_symbol = "[❮](fg:blue)"

[time]
disabled = true
CFG
}

prepare_repo() {
  # Clean previous temp repo; also clean legacy root-level .demo if it exists.
  rm -rf "$DEMO_ROOT"
  if [ -d "$REPO_ROOT/.demo" ] && [ "$REPO_ROOT/.demo" != "$DEMO_ROOT" ]; then
    rm -rf "$REPO_ROOT/.demo"
  fi
  mkdir -p "$DEMO_ROOT"
  export HOME="$DEMO_HOME"
  DEMO_WORK_BASE="$HOME/w"
  rm -rf "$DEMO_WORK_BASE"
  mkdir -p "$DEMO_WORK_BASE"
  DEMO_REPO="$DEMO_WORK_BASE/acme"
  mkdir -p "$DEMO_REPO"
  export DEMO_REPO

  BARE_REMOTE="$DEMO_ROOT/remote.git"
  git init --bare -q "$BARE_REMOTE"

  git -C "$DEMO_REPO" init -q
  git -C "$DEMO_REPO" config user.name "Worktrunk Demo"
  git -C "$DEMO_REPO" config user.email "demo@example.com"
  printf "# Worktrunk demo\n\nThis repo is generated automatically.\n" >"$DEMO_REPO/README.md"
  git -C "$DEMO_REPO" add README.md
  SKIP_DEMO_HOOK=1 git -C "$DEMO_REPO" commit -qm "Initial demo commit"
  git -C "$DEMO_REPO" branch -m main
  git -C "$DEMO_REPO" remote add origin "$BARE_REMOTE"
  git -C "$DEMO_REPO" push -u origin main -q

# Create a simple Rust project with tests
  cat >"$DEMO_REPO/Cargo.toml" <<'CARGO'
[package]
name = "acme"
version = "0.1.0"
edition = "2021"
CARGO
  cat >"$DEMO_REPO/rust-toolchain.toml" <<'TOOLCHAIN'
[toolchain]
channel = "stable"
TOOLCHAIN
  mkdir -p "$DEMO_REPO/src"
  cat >"$DEMO_REPO/src/lib.rs" <<'RUST'
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 2), 4);
    }

    #[test]
    fn test_add_negative() {
        assert_eq!(add(-1, 1), 0);
    }
}
RUST
  echo "/target" >"$DEMO_REPO/.gitignore"
  git -C "$DEMO_REPO" add .gitignore Cargo.toml rust-toolchain.toml src/
  SKIP_DEMO_HOOK=1 git -C "$DEMO_REPO" commit -qm "Add Rust project with tests"
  # Pre-build to create Cargo.lock and cache dependencies
  (cd "$DEMO_REPO" && cargo build --release -q 2>/dev/null)
  git -C "$DEMO_REPO" add Cargo.lock
  SKIP_DEMO_HOOK=1 git -C "$DEMO_REPO" commit -qm "Add Cargo.lock"
  git -C "$DEMO_REPO" push -q

  # Add worktrunk project hooks
  mkdir -p "$DEMO_REPO/.config"
  cat >"$DEMO_REPO/.config/wt.toml" <<'TOML'
[pre-merge-command]
test = "cargo nextest run --no-fail-fast"
TOML
  git -C "$DEMO_REPO" add .config/wt.toml
  SKIP_DEMO_HOOK=1 git -C "$DEMO_REPO" commit -qm "Add project hooks"
  git -C "$DEMO_REPO" push -q

  # Create mock gh CLI for CI status
  mkdir -p "$DEMO_HOME/bin"
  cat >"$DEMO_HOME/bin/gh" <<'GH'
#!/usr/bin/env bash
# Mock gh CLI for demo

if [[ "$1" == "auth" && "$2" == "status" ]]; then
  exit 0
fi

if [[ "$1" == "pr" && "$2" == "list" ]]; then
  branch=""
  for arg in "$@"; do
    if [[ "$prev" == "--head" ]]; then
      branch="$arg"
    fi
    prev="$arg"
  done

  case "$branch" in
    feature/alpha)
      echo '[{"state":"OPEN","headRefOid":"abc123","mergeStateStatus":"CLEAN","statusCheckRollup":[{"status":"COMPLETED","conclusion":"SUCCESS"}],"url":"https://github.com/acme/demo/pull/1"}]'
      ;;
    feature/beta)
      echo '[{"state":"OPEN","headRefOid":"def456","mergeStateStatus":"CLEAN","statusCheckRollup":[{"status":"IN_PROGRESS","conclusion":null}],"url":"https://github.com/acme/demo/pull/2"}]'
      ;;
    feature/hooks)
      echo '[{"state":"OPEN","headRefOid":"ghi789","mergeStateStatus":"CLEAN","statusCheckRollup":[{"status":"COMPLETED","conclusion":"FAILURE"}],"url":"https://github.com/acme/demo/pull/3"}]'
      ;;
    *)
      echo '[]'
      ;;
  esac
  exit 0
fi

if [[ "$1" == "run" && "$2" == "list" ]]; then
  echo '[]'
  exit 0
fi

exit 1
GH
  chmod +x "$DEMO_HOME/bin/gh"

  # Set up user config with LLM and pre-approved commands
  local project_id="${BARE_REMOTE%.git}"
  mkdir -p "$DEMO_HOME/.config/worktrunk"
  cat >"$DEMO_HOME/.config/worktrunk/config.toml" <<TOML
[commit-generation]
command = "llm"
args = ["-m", "claude-haiku-4.5"]

[projects."$project_id"]
approved-commands = ["cargo nextest run --no-fail-fast"]
TOML

  # Create two extra branches (no worktrees) for listing.
  git -C "$DEMO_REPO" branch docs/readme
  git -C "$DEMO_REPO" branch spike/search

  create_branch_and_worktree feature/alpha "notes: alpha" "- Added alpha note"
  create_branch_and_worktree feature/beta "notes: beta" "- Added beta note"
  create_branch_and_worktree feature/hooks "hooks: demo" "- Added hooks demo"
}

create_branch_and_worktree() {
  local branch="$1" label="$2" line="$3"
  local path="$DEMO_WORK_BASE/acme.${branch//\//-}"
  git -C "$DEMO_REPO" checkout -q -b "$branch" main
  printf "%s\n" "$line" >>"$DEMO_REPO/notes.txt"
  git -C "$DEMO_REPO" add notes.txt
  SKIP_DEMO_HOOK=1 git -C "$DEMO_REPO" commit -qm "$label"
  git -C "$DEMO_REPO" push -u origin "$branch" -q
  git -C "$DEMO_REPO" checkout -q main
  git -C "$DEMO_REPO" worktree add -q "$path" "$branch"

  # Add varied states for list output
  case "$branch" in
    feature/alpha)
      echo "// alpha scratch" >"$path/scratch_alpha.rs"               # untracked
      ;;
    feature/beta)
      echo "- beta staged addition" >>"$path/notes.txt"
      git -C "$path" add notes.txt                                   # staged
      ;;
    feature/hooks)
      echo "- hook tweak" >>"$path/notes.txt"
      git -C "$path" add notes.txt && git -C "$path" commit -qm "hook tweak"  # clean after commit, shows history
      ;;
  esac
}

render_tape() {
  sed \
    -e "s|{{DEMO_REPO}}|$DEMO_REPO|g" \
    -e "s|{{DEMO_HOME}}|$DEMO_HOME|g" \
    -e "s|{{REAL_HOME}}|$HOME|g" \
    -e "s|{{STARSHIP_CONFIG}}|$STARSHIP_CONFIG_PATH|g" \
    -e "s|{{OUTPUT_GIF}}|$OUTPUT_GIF|g" \
    "$TAPE_TEMPLATE" >"$TAPE_RENDERED"
}

record_text() {
  mkdir -p "$OUT_DIR"
  DEMO_RAW="$OUT_DIR/run.raw.txt"
  local real_home="$HOME"
  env DEMO_REPO="$DEMO_REPO" RAW_PATH="$DEMO_RAW" bash -lc '
    set -o pipefail
    export LANG=en_US.UTF-8 LC_ALL=en_US.UTF-8
    export RUSTUP_HOME="'"$real_home"'/.rustup"
    export CARGO_HOME="'"$real_home"'/.cargo"
    export HOME="'"$DEMO_HOME"'"
    export PATH="$HOME/bin:$PATH"
    export STARSHIP_CONFIG="'"$STARSHIP_CONFIG_PATH"'"
    export STARSHIP_CACHE="'"$DEMO_ROOT"'"/starship-cache
    mkdir -p "$STARSHIP_CACHE"
    export WT_PROGRESSIVE=false
    export NO_COLOR=1
    export CLICOLOR=0
    eval "$(starship init bash)" >/dev/null 2>&1
    eval "$(wt init bash)" >/dev/null 2>&1
    cd "$DEMO_REPO"
    {
      wt list --branches --full
      wt switch --create feature/reports
      echo "- Q4 report ready" >> notes.md
      wt merge
      wt list --branches --full
    } >"$RAW_PATH" 2>&1
  '
  RAW_PATH="$DEMO_RAW" OUT_DIR="$OUT_DIR" python3 - <<'PY'
import os, re, pathlib
raw = pathlib.Path(os.environ["RAW_PATH"]).read_text(errors="ignore")
# strip ANSI escape sequences and control chars
clean = re.sub(r"\x1B\[[0-9;?]*[A-Za-z]", "", raw)
clean = re.sub(r"[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]", "", clean)
clean = clean.replace("^D", "")
clean = clean.lstrip()
pathlib.Path(os.environ["OUT_DIR"]).joinpath("run.txt").write_text(clean.strip() + "\n")
PY
}

record_vhs() {
  mkdir -p "$OUT_DIR"
  vhs "$TAPE_RENDERED" >"$LOG" 2>&1
}

main() {
  require_bin wt
  require_bin vhs
  require_bin starship
  trap cleanup EXIT

  mkdir -p "$OUT_DIR"
  write_starship_config
  prepare_repo
  record_text
  prepare_repo
  render_tape
  record_vhs

echo "GIF saved to $OUTPUT_GIF"
echo "Text log saved to $OUT_DIR/run.txt"
echo "Log: $LOG"
}

main "$@"
