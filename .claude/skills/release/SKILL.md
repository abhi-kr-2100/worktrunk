---
name: release
description: Worktrunk release workflow. Use when user asks to "do a release", "release a new version", "cut a release", or wants to publish a new version to crates.io and GitHub.
---

# Release Workflow

## Steps

1. **Run tests**: `cargo run -- hook pre-merge --force`
2. **Review CHANGELOG**: Check `## Unreleased` section covers notable changes since last release
3. **Determine version**: Patch (bug fixes), minor (new features), or major (breaking changes)
4. **Update CHANGELOG**: Change `## Unreleased` to `## X.Y.Z`
5. **Bump version**: Update `version` in `Cargo.toml`, run `cargo check` to update `Cargo.lock`
6. **Commit**: `git add -A && git commit -m "Release vX.Y.Z"`
7. **Push to main**: `git push origin <branch>:main`
8. **Tag and push**: `git tag vX.Y.Z && git push origin vX.Y.Z`

The tag push triggers the release workflow which builds binaries and publishes to crates.io.

## CHANGELOG Review

Check commits since last release for missing entries:

```bash
git log v<last-version>..HEAD --oneline
```

Notable changes to document:
- New features or commands
- User-visible behavior changes
- Bug fixes users might encounter
- Breaking changes

Skip: internal refactors, doc-only changes, test additions (unless user-facing like shell completion tests).

## Version Guidelines

- **Patch** (0.1.x → 0.1.y): Bug fixes only
- **Minor** (0.x.0 → 0.y.0): New features, non-breaking changes
- **Major** (x.0.0 → y.0.0): Breaking changes (rare in early development)

Current project status: early release, breaking changes acceptable, optimize for best solution over compatibility.
