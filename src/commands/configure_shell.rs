use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use worktrunk::shell::Shell;

pub struct ConfigureResult {
    pub shell: Shell,
    pub path: PathBuf,
    pub action: ConfigAction,
    pub config_line: String,
}

#[derive(Debug, PartialEq)]
pub enum ConfigAction {
    Added,
    AlreadyExists,
    Created,
    WouldAdd,
    WouldCreate,
}

impl ConfigAction {
    pub fn description(&self) -> &str {
        match self {
            ConfigAction::Added => "Added",
            ConfigAction::AlreadyExists => "Already configured",
            ConfigAction::Created => "Created",
            ConfigAction::WouldAdd => "Would add to",
            ConfigAction::WouldCreate => "Would create",
        }
    }
}

pub fn handle_configure_shell(
    shell_filter: Option<Shell>,
    cmd_prefix: &str,
    dry_run: bool,
) -> Result<Vec<ConfigureResult>, String> {
    let shells = if let Some(shell) = shell_filter {
        vec![shell]
    } else {
        // Try all shells in consistent order
        vec![
            Shell::Bash,
            Shell::Zsh,
            Shell::Fish,
            Shell::Nushell,
            Shell::Powershell,
            Shell::Oil,
            Shell::Elvish,
            Shell::Xonsh,
        ]
    };

    let mut results = Vec::new();
    let mut checked_paths = Vec::new();

    for shell in shells {
        let paths = shell.config_paths(cmd_prefix);

        // Find the first existing config file
        let target_path = paths.iter().find(|p| p.exists());

        // For Fish, also check if the parent directory (conf.d/) exists
        // since we create the file there rather than modifying an existing one
        let has_config_location = if matches!(shell, Shell::Fish) {
            paths
                .first()
                .and_then(|p| p.parent())
                .map(|p| p.exists())
                .unwrap_or(false)
                || target_path.is_some()
        } else {
            target_path.is_some()
        };

        // Track all checked paths for better error messages
        checked_paths.extend(paths.iter().map(|p| (shell, p.clone())));

        // Only configure if explicitly targeting this shell OR if config file/location exists
        let should_configure = shell_filter.is_some() || has_config_location;

        if should_configure {
            let path = target_path.or_else(|| paths.first());
            if let Some(path) = path {
                match configure_shell_file(shell, path, cmd_prefix, dry_run, shell_filter.is_some())
                {
                    Ok(Some(result)) => results.push(result),
                    Ok(None) => {} // No action needed
                    Err(e) => {
                        // For non-critical errors, we could continue with other shells
                        // but for now we'll fail fast
                        return Err(format!("Failed to configure {}: {}", shell, e));
                    }
                }
            }
        }
    }

    if results.is_empty() && shell_filter.is_none() {
        // Provide helpful error message with checked locations
        let example_paths: Vec<String> = checked_paths
            .iter()
            .take(3)
            .map(|(_, p)| p.display().to_string())
            .collect();

        return Err(format!(
            "No shell config files found in $HOME. Checked: {}, and more. Create a config file or use --shell to specify a shell.",
            example_paths.join(", ")
        ));
    }

    Ok(results)
}

fn configure_shell_file(
    shell: Shell,
    path: &Path,
    cmd_prefix: &str,
    dry_run: bool,
    explicit_shell: bool,
) -> Result<Option<ConfigureResult>, String> {
    let config_content = shell.config_line(cmd_prefix);

    // For Fish, we write to a separate conf.d/ file
    if matches!(shell, Shell::Fish) {
        return configure_fish_file(
            shell,
            path,
            &config_content,
            cmd_prefix,
            dry_run,
            explicit_shell,
        );
    }

    // For other shells, check if file exists
    if path.exists() {
        // Read the file and check if our integration already exists
        let file = fs::File::open(path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

        let reader = BufReader::new(file);

        // More precise pattern matching for the eval statement
        let eval_pattern = format!("eval \"$({} init", cmd_prefix);
        let eval_pattern_single_quote = format!("eval '$({} init", cmd_prefix);

        for line in reader.lines() {
            let line = line.map_err(|e| format!("Failed to read line: {}", e))?;
            let trimmed = line.trim();

            // Check for the actual eval statement, not just any mention of "wt init"
            if trimmed.starts_with(&eval_pattern) || trimmed.starts_with(&eval_pattern_single_quote)
            {
                return Ok(Some(ConfigureResult {
                    shell,
                    path: path.to_path_buf(),
                    action: ConfigAction::AlreadyExists,
                    config_line: config_content.to_string(),
                }));
            }
        }

        // Line doesn't exist, add it
        if dry_run {
            return Ok(Some(ConfigureResult {
                shell,
                path: path.to_path_buf(),
                action: ConfigAction::WouldAdd,
                config_line: config_content.to_string(),
            }));
        }

        // Append the line with proper spacing
        let mut file = OpenOptions::new()
            .append(true)
            .open(path)
            .map_err(|e| format!("Failed to open {} for writing: {}", path.display(), e))?;

        // Add blank line before config, then the config line with its own newline
        write!(file, "\n{}\n", config_content)
            .map_err(|e| format!("Failed to write to {}: {}", path.display(), e))?;

        Ok(Some(ConfigureResult {
            shell,
            path: path.to_path_buf(),
            action: ConfigAction::Added,
            config_line: config_content.to_string(),
        }))
    } else {
        // File doesn't exist
        // Only create if explicitly targeting this shell
        if explicit_shell {
            if dry_run {
                return Ok(Some(ConfigureResult {
                    shell,
                    path: path.to_path_buf(),
                    action: ConfigAction::WouldCreate,
                    config_line: config_content.to_string(),
                }));
            }

            // Create parent directories if they don't exist
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    format!("Failed to create directory {}: {}", parent.display(), e)
                })?;
            }

            // Write the config content
            fs::write(path, format!("{}\n", config_content))
                .map_err(|e| format!("Failed to write to {}: {}", path.display(), e))?;

            Ok(Some(ConfigureResult {
                shell,
                path: path.to_path_buf(),
                action: ConfigAction::Created,
                config_line: config_content.to_string(),
            }))
        } else {
            // Don't create config files for shells the user might not use
            Ok(None)
        }
    }
}

fn configure_fish_file(
    shell: Shell,
    path: &Path,
    content: &str,
    cmd_prefix: &str,
    dry_run: bool,
    explicit_shell: bool,
) -> Result<Option<ConfigureResult>, String> {
    // For Fish, we write to conf.d/{cmd_prefix}.fish (separate file)

    // Check if it already exists and has our integration
    if path.exists() {
        let existing_content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

        // Check for the init pattern (either old function-based or new eval-based)
        let init_pattern = format!("{} init", cmd_prefix);
        let has_integration = existing_content.contains(&init_pattern);

        if has_integration {
            return Ok(Some(ConfigureResult {
                shell,
                path: path.to_path_buf(),
                action: ConfigAction::AlreadyExists,
                config_line: content.to_string(),
            }));
        }
    }

    // File doesn't exist or doesn't have our integration
    // For Fish, create if parent directory exists or if explicitly targeting this shell
    // This is different from other shells because Fish uses conf.d/ which may exist
    // even if the specific wt.fish file doesn't
    if !explicit_shell && !path.exists() {
        // Check if parent directory exists
        let parent_exists = path.parent().map(|p| p.exists()).unwrap_or(false);
        if !parent_exists {
            return Ok(None);
        }
    }

    if dry_run {
        return Ok(Some(ConfigureResult {
            shell,
            path: path.to_path_buf(),
            action: if path.exists() {
                ConfigAction::WouldAdd
            } else {
                ConfigAction::WouldCreate
            },
            config_line: content.to_string(),
        }));
    }

    // Create parent directories if they don't exist
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
    }

    // Write the config content with newline
    fs::write(path, format!("{}\n", content))
        .map_err(|e| format!("Failed to write to {}: {}", path.display(), e))?;

    Ok(Some(ConfigureResult {
        shell,
        path: path.to_path_buf(),
        action: ConfigAction::Created,
        config_line: content.to_string(),
    }))
}
