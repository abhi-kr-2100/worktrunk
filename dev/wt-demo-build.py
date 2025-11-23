#!/usr/bin/env python3
"""Build script for worktrunk demo GIF and text output."""

import os
import re
import shutil
import subprocess
from datetime import datetime, timedelta
from pathlib import Path

SCRIPT_DIR = Path(__file__).parent
REPO_ROOT = SCRIPT_DIR.parent
DEMO_DIR = SCRIPT_DIR / "wt-demo"
FIXTURES_DIR = DEMO_DIR / "fixtures"
OUT_DIR = DEMO_DIR / "out"
DEMO_ROOT = OUT_DIR / ".demo"
DEMO_HOME = Path(os.environ.get("DEMO_HOME", DEMO_ROOT))
TAPE_TEMPLATE = DEMO_DIR / "demo.tape"
TAPE_RENDERED = OUT_DIR / ".rendered.tape"
STARSHIP_CONFIG = OUT_DIR / "starship.toml"
OUTPUT_GIF = OUT_DIR / "wt-demo.gif"
LOG = OUT_DIR / "record.log"

REAL_HOME = Path.home()
DEMO_WORK_BASE = DEMO_HOME / "w"
DEMO_REPO = DEMO_WORK_BASE / "acme"
BARE_REMOTE = DEMO_ROOT / "remote.git"


def run(cmd, cwd=None, env=None, check=True, capture=False):
    """Run a command."""
    result = subprocess.run(
        cmd, cwd=cwd, env=env, check=check,
        capture_output=capture, text=True
    )
    return result.stdout if capture else None


def git(args, cwd=None, env=None):
    """Run git command."""
    run(["git"] + args, cwd=cwd, env=env)


def commit_dated(repo, message, offset):
    """Commit with a date offset like '7d' or '2H'."""
    now = datetime.now()
    if offset.endswith("d"):
        delta = timedelta(days=int(offset[:-1]))
    elif offset.endswith("H"):
        delta = timedelta(hours=int(offset[:-1]))
    else:
        raise ValueError(f"Unknown offset format: {offset}")

    date_str = (now - delta).strftime("%Y-%m-%dT%H:%M:%S")
    env = os.environ.copy()
    env["GIT_AUTHOR_DATE"] = date_str
    env["GIT_COMMITTER_DATE"] = date_str
    env["SKIP_DEMO_HOOK"] = "1"
    git(["-C", str(repo), "commit", "-qm", message], env=env)


def prepare_repo():
    """Set up the demo repository with branches and worktrees."""
    # Clean previous
    shutil.rmtree(DEMO_ROOT, ignore_errors=True)
    legacy = REPO_ROOT / ".demo"
    if legacy.exists() and legacy != DEMO_ROOT:
        shutil.rmtree(legacy)

    DEMO_ROOT.mkdir(parents=True)
    DEMO_WORK_BASE.mkdir(parents=True)
    DEMO_REPO.mkdir(parents=True)

    # Init bare remote
    run(["git", "init", "--bare", "-q", str(BARE_REMOTE)])

    # Init main repo
    git(["-C", str(DEMO_REPO), "init", "-q"])
    git(["-C", str(DEMO_REPO), "config", "user.name", "Worktrunk Demo"])
    git(["-C", str(DEMO_REPO), "config", "user.email", "demo@example.com"])
    git(["-C", str(DEMO_REPO), "config", "commit.gpgsign", "false"])

    # Initial commit
    (DEMO_REPO / "README.md").write_text("# Worktrunk demo\n\nThis repo is generated automatically.\n")
    git(["-C", str(DEMO_REPO), "add", "README.md"])
    commit_dated(DEMO_REPO, "Initial demo commit", "7d")
    git(["-C", str(DEMO_REPO), "branch", "-m", "main"])
    git(["-C", str(DEMO_REPO), "remote", "add", "origin", str(BARE_REMOTE)])
    git(["-C", str(DEMO_REPO), "push", "-u", "origin", "main", "-q"])

    # Rust project
    (DEMO_REPO / "Cargo.toml").write_text(
        "[package]\nname = \"acme\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n"
    )
    (DEMO_REPO / "src").mkdir()
    shutil.copy(FIXTURES_DIR / "lib.rs", DEMO_REPO / "src" / "lib.rs")
    (DEMO_REPO / ".gitignore").write_text("/target\n")
    git(["-C", str(DEMO_REPO), "add", ".gitignore", "Cargo.toml", "src/"])
    commit_dated(DEMO_REPO, "Add Rust project with tests", "6d")

    # Build to create Cargo.lock
    run(["cargo", "build", "--release", "-q"], cwd=DEMO_REPO, check=False)
    git(["-C", str(DEMO_REPO), "add", "Cargo.lock"])
    commit_dated(DEMO_REPO, "Add Cargo.lock", "6d")
    git(["-C", str(DEMO_REPO), "push", "-q"])

    # Project hooks
    (DEMO_REPO / ".config").mkdir()
    (DEMO_REPO / ".config" / "wt.toml").write_text(
        '[pre-merge-command]\ntest = "cargo nextest run"\n'
    )
    git(["-C", str(DEMO_REPO), "add", ".config/wt.toml"])
    commit_dated(DEMO_REPO, "Add project hooks", "5d")
    git(["-C", str(DEMO_REPO), "push", "-q"])

    # Mock gh CLI
    bin_dir = DEMO_HOME / "bin"
    bin_dir.mkdir(parents=True, exist_ok=True)
    gh_mock = bin_dir / "gh"
    shutil.copy(FIXTURES_DIR / "gh-mock.sh", gh_mock)
    gh_mock.chmod(0o755)

    # Build wt binary
    run(["cargo", "build", "--quiet"], cwd=REPO_ROOT)

    # User config
    config_dir = DEMO_HOME / ".config" / "worktrunk"
    config_dir.mkdir(parents=True)
    project_id = str(BARE_REMOTE).removesuffix(".git")
    (config_dir / "config.toml").write_text(f'''[commit-generation]
command = "llm"
args = ["-m", "claude-haiku-4.5"]

[projects."{project_id}"]
approved-commands = ["cargo nextest run"]
''')

    # Extra branches (no worktrees)
    git(["-C", str(DEMO_REPO), "branch", "docs/readme"])
    git(["-C", str(DEMO_REPO), "branch", "spike/search"])

    # Create feature branches
    create_branch_alpha()
    create_branch_beta()

    # Commit to main after beta so beta is behind
    readme = DEMO_REPO / "README.md"
    readme.write_text(readme.read_text() + "# Development\nSee CONTRIBUTING.md for guidelines.\n")
    (DEMO_REPO / "notes.md").write_text("# Notes\n")
    git(["-C", str(DEMO_REPO), "add", "README.md", "notes.md"])
    commit_dated(DEMO_REPO, "docs: add development section", "1d")
    git(["-C", str(DEMO_REPO), "push", "-q"])

    create_branch_hooks()


def create_branch_alpha():
    """Create alpha branch with large diff and unpushed commits."""
    branch = "alpha"
    path = DEMO_WORK_BASE / f"acme.{branch}"

    git(["-C", str(DEMO_REPO), "checkout", "-q", "-b", branch, "main"])

    # Initial README changes
    (DEMO_REPO / "README.md").write_text('''# Worktrunk demo

A demo repository for showcasing worktrunk features.

## Features

- Fast worktree switching
- Integrated merge workflow
- Pre-merge test hooks
- LLM commit messages

## Getting Started

Run `wt list` to see all worktrees.
''')
    git(["-C", str(DEMO_REPO), "add", "README.md"])
    commit_dated(DEMO_REPO, "docs: expand README", "3d")

    # More commits
    readme = DEMO_REPO / "README.md"
    readme.write_text(readme.read_text() + "\n# Contributing\nPRs welcome!\n")
    git(["-C", str(DEMO_REPO), "add", "README.md"])
    commit_dated(DEMO_REPO, "docs: add contributing section", "3d")

    readme.write_text(readme.read_text() + "\n# License\nMIT\n")
    git(["-C", str(DEMO_REPO), "add", "README.md"])
    commit_dated(DEMO_REPO, "docs: add license", "3d")

    git(["-C", str(DEMO_REPO), "push", "-u", "origin", branch, "-q"])
    git(["-C", str(DEMO_REPO), "checkout", "-q", "main"])
    git(["-C", str(DEMO_REPO), "worktree", "add", "-q", str(path), branch])

    # Unpushed commit
    readme = path / "README.md"
    readme.write_text(readme.read_text() + "# FAQ\n")
    git(["-C", str(path), "add", "README.md"])
    commit_dated(path, "docs: add FAQ section", "3d")

    # Working tree changes - large diff
    shutil.copy(FIXTURES_DIR / "alpha-readme.md", path / "README.md")
    (path / "scratch.rs").write_text("// scratch\n")


def create_branch_beta():
    """Create beta branch with staged changes."""
    branch = "beta"
    path = DEMO_WORK_BASE / f"acme.{branch}"

    git(["-C", str(DEMO_REPO), "checkout", "-q", "-b", branch, "main"])
    git(["-C", str(DEMO_REPO), "push", "-u", "origin", branch, "-q"])
    git(["-C", str(DEMO_REPO), "checkout", "-q", "main"])
    git(["-C", str(DEMO_REPO), "worktree", "add", "-q", str(path), branch])

    # Staged new file
    (path / "notes.txt").write_text("# TODO\n- Add caching\n")
    git(["-C", str(path), "add", "notes.txt"])


def create_branch_hooks():
    """Create hooks branch with refactored lib.rs."""
    branch = "hooks"
    path = DEMO_WORK_BASE / f"acme.{branch}"

    git(["-C", str(DEMO_REPO), "checkout", "-q", "-b", branch, "main"])
    shutil.copy(FIXTURES_DIR / "lib-hooks.rs", DEMO_REPO / "src" / "lib.rs")
    git(["-C", str(DEMO_REPO), "add", "src/lib.rs"])
    commit_dated(DEMO_REPO, "feat: add math operations, consolidate tests", "2H")

    # No push - no upstream
    git(["-C", str(DEMO_REPO), "checkout", "-q", "main"])
    git(["-C", str(DEMO_REPO), "worktree", "add", "-q", str(path), branch])

    # Staged then modified
    lib_rs = path / "src" / "lib.rs"
    lib_rs.write_text(lib_rs.read_text() + "// Division coming soon\n")
    git(["-C", str(path), "add", "src/lib.rs"])
    lib_rs.write_text(lib_rs.read_text() + "// TODO: add division\n")


def render_tape():
    """Render the VHS tape template with variables."""
    template = TAPE_TEMPLATE.read_text()
    rendered = template.replace("{{DEMO_REPO}}", str(DEMO_REPO))
    rendered = rendered.replace("{{DEMO_HOME}}", str(DEMO_HOME))
    rendered = rendered.replace("{{REAL_HOME}}", str(REAL_HOME))
    rendered = rendered.replace("{{STARSHIP_CONFIG}}", str(STARSHIP_CONFIG))
    rendered = rendered.replace("{{OUTPUT_GIF}}", str(OUTPUT_GIF))
    rendered = rendered.replace("{{TARGET_DEBUG}}", str(REPO_ROOT / "target" / "debug"))
    TAPE_RENDERED.write_text(rendered)


def record_text():
    """Record text output by running demo commands."""
    # Extract commands from tape
    commands = []
    for line in TAPE_TEMPLATE.read_text().splitlines():
        if line.startswith("Type "):
            cmd = line[5:].strip().strip('"').strip("'")
            # Skip setup commands
            if not any(cmd.startswith(p) for p in ["export ", "eval ", "cd ", "clear", "exit"]):
                commands.append(cmd)

    # Set up environment
    env = os.environ.copy()
    env.update({
        "LANG": "en_US.UTF-8",
        "LC_ALL": "en_US.UTF-8",
        "COLUMNS": "160",
        "RUSTUP_HOME": str(REAL_HOME / ".rustup"),
        "CARGO_HOME": str(REAL_HOME / ".cargo"),
        "HOME": str(DEMO_HOME),
        "PATH": f"{REPO_ROOT}/target/debug:{DEMO_HOME}/bin:{os.environ['PATH']}",
        "STARSHIP_CONFIG": str(STARSHIP_CONFIG),
        "STARSHIP_CACHE": str(DEMO_ROOT / "starship-cache"),
        "WT_PROGRESSIVE": "false",
        "NO_COLOR": "1",
        "CLICOLOR": "0",
        "NEXTEST_STATUS_LEVEL": "none",
        "NEXTEST_FINAL_STATUS_LEVEL": "flaky",
        "NEXTEST_NO_FAIL_FAST": "1",
    })

    (DEMO_ROOT / "starship-cache").mkdir(exist_ok=True)

    # Run commands and capture output using bash with shell init
    script = '''
eval "$(wt config shell init bash)" >/dev/null 2>&1
'''
    for cmd in commands:
        script += f'{cmd}\n'
        if cmd.startswith("wt merge"):
            script += 'sleep 2\n'

    result = subprocess.run(
        ["bash", "-c", script],
        cwd=DEMO_REPO, env=env,
        capture_output=True, text=True
    )
    output = [result.stdout + result.stderr]

    # Clean output
    raw = "".join(output)
    clean = re.sub(r"\x1B\[[0-9;?]*[A-Za-z]", "", raw)  # Strip ANSI
    clean = re.sub(r"[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]", "", clean)  # Strip control chars
    clean = clean.replace("^D", "")

    (OUT_DIR / "run.txt").write_text(clean)


def record_vhs():
    """Record the demo GIF using VHS."""
    with open(LOG, "w") as f:
        run(["vhs", str(TAPE_RENDERED)], check=True)


def main():
    # Check dependencies
    for cmd in ["wt", "vhs", "starship"]:
        if not shutil.which(cmd):
            raise SystemExit(f"Missing dependency: {cmd}")

    OUT_DIR.mkdir(parents=True, exist_ok=True)

    # Copy starship config
    shutil.copy(FIXTURES_DIR / "starship.toml", STARSHIP_CONFIG)

    prepare_repo()
    record_text()
    render_tape()
    record_vhs()

    # Cleanup
    TAPE_RENDERED.unlink(missing_ok=True)

    print(f"GIF saved to {OUTPUT_GIF}")
    print(f"Text log saved to {OUT_DIR / 'run.txt'}")
    print(f"Log: {LOG}")


if __name__ == "__main__":
    main()
