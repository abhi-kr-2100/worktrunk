//! Template expansion utilities for worktrunk
//!
//! Uses minijinja for all template rendering with automatic shell escaping.
//! All templates support Jinja2 syntax including filters, conditionals, and loops.

use minijinja::Environment;

/// Sanitize a branch name for use in filesystem paths.
///
/// Replaces path separators (`/` and `\`) with dashes to prevent directory traversal
/// and ensure the branch name is a single path component.
///
/// # Examples
/// ```
/// use worktrunk::config::sanitize_branch_name;
///
/// assert_eq!(sanitize_branch_name("feature/foo"), "feature-foo");
/// assert_eq!(sanitize_branch_name("user\\task"), "user-task");
/// assert_eq!(sanitize_branch_name("simple-branch"), "simple-branch");
/// ```
pub fn sanitize_branch_name(branch: &str) -> String {
    branch.replace(['/', '\\'], "-")
}

/// Expand template variables using minijinja
///
/// All templates support:
/// - `{{ main_worktree }}` - Main worktree directory name
/// - `{{ branch }}` - Branch name (sanitized: slashes → dashes)
/// - `{{ repo }}` - Repository name (alias for main_worktree for consistency)
///
/// Additional variables can be provided via the `extra` parameter.
///
/// Variables are automatically shell-escaped for safe use in commands.
/// Use the `|e` or `|escape` filter for additional escaping if needed.
///
/// # Examples
/// ```
/// use worktrunk::config::expand_template;
/// use std::collections::HashMap;
///
/// // Simple variable substitution
/// let result = expand_template("path/{{ main_worktree }}/{{ branch }}", "myrepo", "feature/foo", &HashMap::new()).unwrap();
/// assert_eq!(result, "path/myrepo/feature-foo");
///
/// // Using conditionals
/// let result = expand_template("{% if branch == 'main' %}production{% else %}development{% endif %}", "myrepo", "main", &HashMap::new()).unwrap();
/// assert_eq!(result, "production");
/// ```
pub fn expand_template(
    template: &str,
    main_worktree: &str,
    branch: &str,
    extra: &std::collections::HashMap<&str, &str>,
) -> Result<String, String> {
    use shell_escape::escape;
    use std::borrow::Cow;

    // Sanitize branch name by replacing path separators
    let safe_branch = sanitize_branch_name(branch);

    // Shell-escape all variables
    let escaped_worktree = escape(Cow::Borrowed(main_worktree)).to_string();
    let escaped_branch = escape(Cow::Borrowed(safe_branch.as_str())).to_string();

    // Build context map with shell-escaped values
    let mut context = std::collections::HashMap::new();
    context.insert("branch".to_string(), minijinja::Value::from(escaped_branch));
    context.insert(
        "main_worktree".to_string(),
        minijinja::Value::from(escaped_worktree.clone()),
    );
    context.insert("repo".to_string(), minijinja::Value::from(escaped_worktree));

    for (key, value) in extra {
        context.insert(
            key.to_string(),
            minijinja::Value::from(escape(Cow::Borrowed(*value)).to_string()),
        );
    }

    // Render template with minijinja
    let mut env = Environment::new();
    // Preserve trailing newlines in templates (important for multiline shell commands)
    env.set_keep_trailing_newline(true);
    let tmpl = env
        .template_from_str(template)
        .map_err(|e| format!("Template syntax error: {}", e))?;

    tmpl.render(minijinja::Value::from_object(context))
        .map_err(|e| format!("Template render error: {}", e))
}

/// Expand command template variables using minijinja
///
/// Convenience function for expanding command templates with common variables.
///
/// Supported variables:
/// - `{{ repo }}` - Repository name
/// - `{{ branch }}` - Branch name (sanitized: slashes → dashes)
/// - `{{ worktree }}` - Absolute path to the worktree
/// - `{{ worktree_name }}` - Worktree directory name (basename of worktree path)
/// - `{{ repo_root }}` - Absolute path to the main repository root
/// - `{{ default_branch }}` - Default branch name (e.g., "main")
/// - `{{ commit }}` - Current HEAD commit SHA (full 40-character hash)
/// - `{{ short_commit }}` - Current HEAD commit SHA (short 7-character hash)
/// - `{{ remote }}` - Primary remote name (e.g., "origin")
/// - `{{ upstream }}` - Upstream tracking branch (e.g., "origin/feature"), if configured
/// - `{{ target }}` - Target branch (for merge commands, optional)
///
/// # Examples
/// ```
/// use worktrunk::config::expand_command_template;
/// use std::path::Path;
///
/// let cmd = expand_command_template(
///     "cp {{ repo_root }}/target {{ worktree }}/target",
///     "myrepo",
///     "feature",
///     Path::new("/path/to/worktree"),
///     Path::new("/path/to/repo"),
///     None,
/// ).unwrap();
/// ```
pub fn expand_command_template(
    command: &str,
    repo_name: &str,
    branch: &str,
    worktree_path: &std::path::Path,
    repo_root: &std::path::Path,
    target_branch: Option<&str>,
) -> Result<String, String> {
    let mut extra = std::collections::HashMap::new();
    let worktree_str = worktree_path.to_string_lossy();
    let repo_root_str = repo_root.to_string_lossy();
    extra.insert("worktree", worktree_str.as_ref());
    extra.insert("repo_root", repo_root_str.as_ref());
    if let Some(target) = target_branch {
        extra.insert("target", target);
    }

    expand_template(command, repo_name, branch, &extra)
}
