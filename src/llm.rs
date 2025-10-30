use std::io::Write;
use std::process::{self, Stdio};
use worktrunk::config::CommitGenerationConfig;
use worktrunk::git::{GitError, Repository};

/// Default template for commit message prompts
const DEFAULT_TEMPLATE: &str = r#"Format
- First line: <50 chars, present tense, describes WHAT and WHY (not HOW).
- Blank line after first line.
- Optional details with proper line breaks explaining context. Commits with more substantial changes should have more details.
- Return ONLY the formatted message without quotes, code blocks, or preamble.

Style
- Do not give normative statements or otherwise speculate on why the change was made.
- Broadly match the style of the previous commit messages.
  - For example, if they're in conventional commit format, use conventional commits; if they're not, don't use conventional commits.

The context contains:
- <git-diff> with the staged changes. This is the ONLY content you should base your message on.
- <git-info> with branch name and recent commit message titles for style reference ONLY. DO NOT use their content to inform your message.

---
The following is the context for your task:
---
<git-diff>
```
{git-diff}
```
</git-diff>

<git-info>
  <current-branch>{branch}</current-branch>
{recent-commits}
</git-info>
"#;

/// Execute an LLM command with the given prompt via stdin.
///
/// This is the canonical way to execute LLM commands in this codebase.
/// All LLM execution should go through this function to maintain consistency.
fn execute_llm_command(
    command: &str,
    args: &[String],
    prompt: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // Build command args
    let mut cmd = process::Command::new(command);
    cmd.args(args);

    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Log execution
    log::debug!("$ {} {}", command, args.join(" "));
    log::debug!("  Prompt (stdin):");
    for line in prompt.lines() {
        log::debug!("    {}", line);
    }

    let mut child = cmd.spawn()?;

    // Write prompt to stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(prompt.as_bytes())?;
        // stdin is dropped here, closing the pipe
    }

    let output = child.wait_with_output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("LLM command failed: {}", stderr).into());
    }

    let message = String::from_utf8_lossy(&output.stdout).trim().to_owned();

    if message.is_empty() {
        return Err("LLM returned empty message".into());
    }

    Ok(message)
}

/// Format recent commits for template expansion
fn format_recent_commits(commits: Option<&Vec<String>>) -> String {
    match commits {
        Some(commits) if !commits.is_empty() => {
            let mut result = String::from("  <previous-commit-message-titles>\n");
            for commit in commits {
                result.push_str(&format!(
                    "    <previous-commit-message-title>{}</previous-commit-message-title>\n",
                    commit
                ));
            }
            result.push_str("  </previous-commit-message-titles>");
            result
        }
        _ => String::new(),
    }
}

/// Build the commit prompt from config template or default
fn build_commit_prompt(
    config: &CommitGenerationConfig,
    diff: &str,
    branch: &str,
    recent_commits: Option<&Vec<String>>,
    repo_name: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // Get template source
    let template = match (&config.template, &config.template_file) {
        (Some(inline), None) => inline.clone(),
        (None, Some(path)) => {
            let expanded_path = worktrunk::config::expand_tilde(path);
            std::fs::read_to_string(&expanded_path).map_err(|e| {
                format!(
                    "Failed to read template-file '{}': {}",
                    expanded_path.display(),
                    e
                )
            })?
        }
        (None, None) => DEFAULT_TEMPLATE.to_string(),
        (Some(_), Some(_)) => {
            unreachable!("Config validation should prevent both template and template-file")
        }
    };

    // Validate non-empty
    if template.trim().is_empty() {
        return Err("Template is empty".into());
    }

    // Format recent commits
    let recent_commits_formatted = format_recent_commits(recent_commits);

    // Expand variables
    let expanded = worktrunk::config::expand_commit_template(
        &template,
        diff,
        branch,
        &recent_commits_formatted,
        repo_name,
    );

    Ok(expanded)
}

pub fn generate_commit_message(
    commit_generation_config: &CommitGenerationConfig,
) -> Result<String, GitError> {
    // Check if commit generation is configured (non-empty command)
    if let Some(ref command) = commit_generation_config.command
        && !command.trim().is_empty()
    {
        // Commit generation is explicitly configured - fail if it doesn't work
        return try_generate_commit_message(
            command,
            &commit_generation_config.args,
            commit_generation_config,
        )
        .map_err(|e| {
            GitError::CommandFailed(format!(
                "Commit generation command '{}' failed: {}",
                command, e
            ))
        });
    }

    // Fallback: simple deterministic commit message (only when not configured)
    Ok("WIP: Auto-commit before merge".to_string())
}

fn try_generate_commit_message(
    command: &str,
    args: &[String],
    config: &CommitGenerationConfig,
) -> Result<String, Box<dyn std::error::Error>> {
    let repo = Repository::current();

    // Get staged diff
    let diff_output = repo.run_command(&["--no-pager", "diff", "--staged"])?;

    // Get current branch
    let current_branch = repo.current_branch()?.unwrap_or_else(|| "HEAD".to_string());

    // Get repo name from directory
    let repo_root = repo.worktree_root()?;
    let repo_name = repo_root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repo");

    // Get recent commit messages for style reference
    let recent_commits = repo
        .run_command(&["log", "--pretty=format:%s", "-n", "5", "--no-merges"])
        .ok()
        .and_then(|output| {
            if output.trim().is_empty() {
                None
            } else {
                Some(output.lines().map(String::from).collect::<Vec<_>>())
            }
        });

    // Build prompt from template
    let prompt = build_commit_prompt(
        config,
        &diff_output,
        &current_branch,
        recent_commits.as_ref(),
        repo_name,
    )?;

    execute_llm_command(command, args, &prompt)
}

pub fn generate_squash_message(
    target_branch: &str,
    subjects: &[String],
    commit_generation_config: &CommitGenerationConfig,
) -> Result<String, Box<dyn std::error::Error>> {
    // Check if commit generation is configured (non-empty command)
    if let Some(ref command) = commit_generation_config.command
        && !command.trim().is_empty()
    {
        // Commit generation is explicitly configured - fail if it doesn't work
        return try_generate_llm_message(
            target_branch,
            subjects,
            command,
            &commit_generation_config.args,
        );
    }

    // Fallback: deterministic commit message (only when not configured)
    let mut commit_message = format!("Squash commits from {}\n\n", target_branch);
    commit_message.push_str("Combined commits:\n");
    for subject in subjects.iter().rev() {
        // Reverse so they're in chronological order
        commit_message.push_str(&format!("- {}\n", subject));
    }
    Ok(commit_message)
}

fn try_generate_llm_message(
    target_branch: &str,
    subjects: &[String],
    command: &str,
    args: &[String],
) -> Result<String, Box<dyn std::error::Error>> {
    // Build context prompt
    let mut context = format!(
        "Squashing commits on current branch since branching from {}\n\n",
        target_branch
    );
    context.push_str("Commits being combined:\n");
    for subject in subjects.iter().rev() {
        context.push_str(&format!("- {}\n", subject));
    }

    let prompt = "Generate a conventional commit message (feat/fix/docs/style/refactor) that combines these changes into one cohesive message. Output only the commit message without any explanation.";
    let full_prompt = format!("{}\n\n{}", context, prompt);

    execute_llm_command(command, args, &full_prompt)
}
