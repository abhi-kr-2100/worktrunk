use askama::Template;
use std::fmt;
use std::path::PathBuf;

/// Supported shells
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Shell {
    Bash,
    Elvish,
    Fish,
    Nushell,
    Oil,
    Powershell,
    Xonsh,
    Zsh,
}

impl std::str::FromStr for Shell {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "bash" => Ok(Shell::Bash),
            "elvish" => Ok(Shell::Elvish),
            "fish" => Ok(Shell::Fish),
            "nushell" => Ok(Shell::Nushell),
            "oil" => Ok(Shell::Oil),
            "powershell" => Ok(Shell::Powershell),
            "xonsh" => Ok(Shell::Xonsh),
            "zsh" => Ok(Shell::Zsh),
            _ => Err(format!("Unsupported shell: {}", s)),
        }
    }
}

impl fmt::Display for Shell {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Shell::Bash => write!(f, "bash"),
            Shell::Elvish => write!(f, "elvish"),
            Shell::Fish => write!(f, "fish"),
            Shell::Nushell => write!(f, "nushell"),
            Shell::Oil => write!(f, "oil"),
            Shell::Powershell => write!(f, "powershell"),
            Shell::Xonsh => write!(f, "xonsh"),
            Shell::Zsh => write!(f, "zsh"),
        }
    }
}

impl Shell {
    /// Returns true if this shell supports completion generation
    pub fn supports_completion(&self) -> bool {
        matches!(self, Self::Bash | Self::Fish | Self::Zsh | Self::Oil)
    }

    /// Returns the standard config file paths for this shell
    ///
    /// Returns paths in order of preference. The first existing file should be used.
    /// For Fish, the cmd_prefix is used to name the conf.d file.
    pub fn config_paths(&self, cmd_prefix: &str) -> Vec<PathBuf> {
        let home = PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string()));

        match self {
            Self::Bash => {
                // macOS uses .bash_profile, Linux typically uses .bashrc
                if cfg!(target_os = "macos") {
                    vec![home.join(".bash_profile"), home.join(".profile")]
                } else {
                    vec![home.join(".bashrc"), home.join(".bash_profile")]
                }
            }
            Self::Zsh => {
                let zdotdir = std::env::var("ZDOTDIR")
                    .map(PathBuf::from)
                    .unwrap_or_else(|_| home.clone());
                vec![zdotdir.join(".zshrc")]
            }
            Self::Fish => {
                // For fish, we write to conf.d/ which is auto-sourced
                // Use cmd_prefix in the filename
                vec![
                    home.join(".config")
                        .join("fish")
                        .join("conf.d")
                        .join(format!("{}.fish", cmd_prefix)),
                ]
            }
            Self::Nushell => {
                vec![home.join(".config").join("nushell").join("config.nu")]
            }
            Self::Powershell => {
                if cfg!(target_os = "windows") {
                    let userprofile = PathBuf::from(
                        std::env::var("USERPROFILE").unwrap_or_else(|_| ".".to_string()),
                    );
                    vec![
                        userprofile
                            .join("Documents")
                            .join("PowerShell")
                            .join("Microsoft.PowerShell_profile.ps1"),
                    ]
                } else {
                    vec![
                        home.join(".config")
                            .join("powershell")
                            .join("Microsoft.PowerShell_profile.ps1"),
                    ]
                }
            }
            Self::Oil => {
                vec![home.join(".config").join("oil").join("oshrc")]
            }
            Self::Elvish => {
                vec![home.join(".config").join("elvish").join("rc.elv")]
            }
            Self::Xonsh => {
                vec![home.join(".xonshrc")]
            }
        }
    }

    /// Returns the line to add to the config file for shell integration
    ///
    /// All shells use an eval-like pattern now.
    pub fn config_line(&self, cmd_prefix: &str) -> String {
        match self {
            Self::Fish => {
                // Fish uses 'source' instead of 'eval'
                format!("{} init {} | source", cmd_prefix, self)
            }
            _ => {
                // All other shells use eval pattern
                format!("eval \"$({} init {})\"", cmd_prefix, self)
            }
        }
    }
}

/// Shell integration configuration
pub struct ShellInit {
    pub shell: Shell,
    pub cmd_prefix: String,
}

impl ShellInit {
    pub fn new(shell: Shell, cmd_prefix: String) -> Self {
        Self { shell, cmd_prefix }
    }

    /// Generate shell integration code
    pub fn generate(&self) -> Result<String, askama::Error> {
        match self.shell {
            Shell::Bash | Shell::Zsh | Shell::Oil => {
                let template = BashTemplate {
                    shell_name: self.shell.to_string(),
                    cmd_prefix: &self.cmd_prefix,
                };
                template.render()
            }
            Shell::Fish => {
                let template = FishTemplate {
                    cmd_prefix: &self.cmd_prefix,
                };
                template.render()
            }
            Shell::Nushell => {
                let template = NushellTemplate {
                    cmd_prefix: &self.cmd_prefix,
                };
                template.render()
            }
            Shell::Powershell => {
                let template = PowershellTemplate {
                    cmd_prefix: &self.cmd_prefix,
                };
                template.render()
            }
            Shell::Elvish => {
                let template = ElvishTemplate {
                    cmd_prefix: &self.cmd_prefix,
                };
                template.render()
            }
            Shell::Xonsh => {
                let template = XonshTemplate {
                    cmd_prefix: &self.cmd_prefix,
                };
                template.render()
            }
        }
    }
}

/// Bash/Zsh shell template
#[derive(Template)]
#[template(path = "bash.sh", escape = "none")]
struct BashTemplate<'a> {
    shell_name: String,
    cmd_prefix: &'a str,
}

/// Fish shell template
#[derive(Template)]
#[template(path = "fish.fish", escape = "none")]
struct FishTemplate<'a> {
    cmd_prefix: &'a str,
}

/// Nushell shell template
#[derive(Template)]
#[template(path = "nushell.nu", escape = "none")]
struct NushellTemplate<'a> {
    cmd_prefix: &'a str,
}

/// PowerShell template
#[derive(Template)]
#[template(path = "powershell.ps1", escape = "none")]
struct PowershellTemplate<'a> {
    cmd_prefix: &'a str,
}

/// Elvish shell template
#[derive(Template)]
#[template(path = "elvish.elv", escape = "none")]
struct ElvishTemplate<'a> {
    cmd_prefix: &'a str,
}

/// Xonsh shell template
#[derive(Template)]
#[template(path = "xonsh.xsh", escape = "none")]
struct XonshTemplate<'a> {
    cmd_prefix: &'a str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_from_str() {
        assert!(matches!("bash".parse::<Shell>(), Ok(Shell::Bash)));
        assert!(matches!("BASH".parse::<Shell>(), Ok(Shell::Bash)));
        assert!(matches!("fish".parse::<Shell>(), Ok(Shell::Fish)));
        assert!(matches!("zsh".parse::<Shell>(), Ok(Shell::Zsh)));
        assert!("invalid".parse::<Shell>().is_err());
    }
}
