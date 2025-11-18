use super::{TestRepo, wt_command};
use insta_cmd::get_cargo_bin;
use std::process::Command;

/// Get the path to the dev-detach helper binary.
/// This binary calls setsid() before exec'ing the shell, detaching it from
/// any controlling terminal and preventing PTY-related hangs.
///
/// Automatically builds dev-detach if it doesn't exist, so `cargo test` works
/// with zero setup.
fn get_dev_detach_bin() -> std::path::PathBuf {
    use std::sync::OnceLock;

    // Build dev-detach once on first call if needed
    static BUILT: OnceLock<()> = OnceLock::new();
    BUILT.get_or_init(|| {
        // Check if dev-detach binary exists in target directory
        let expected_path = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("dev-detach")))
            .filter(|p| p.exists());

        if expected_path.is_none() {
            eprintln!("Building dev-detach helper binary...");
            let status = Command::new("cargo")
                .args(["build", "-p", "dev-detach"])
                .status()
                .expect("Failed to run cargo build");

            if !status.success() {
                panic!("Failed to build dev-detach");
            }
        }
    });

    let bin_path = get_cargo_bin("dev-detach");

    // Verify the binary actually exists and is executable
    if !bin_path.exists() {
        panic!(
            "get_cargo_bin returned non-existent path: {}",
            bin_path.display()
        );
    }

    // Check if the binary is executable (Unix-specific)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = std::fs::metadata(&bin_path) {
            let permissions = metadata.permissions();
            if permissions.mode() & 0o111 == 0 {
                panic!(
                    "dev-detach binary is not executable: {} (mode: {:o})",
                    bin_path.display(),
                    permissions.mode()
                );
            }
        }
    }

    // Log the path for debugging (only visible when tests fail)
    eprintln!("Using dev-detach binary at: {}", bin_path.display());

    bin_path
}

/// Map shell display names to actual binaries.
pub fn get_shell_binary(shell: &str) -> &str {
    match shell {
        "nushell" => "nu",
        "powershell" => "pwsh",
        "oil" => "osh",
        _ => shell,
    }
}

/// Execute a script in the given shell with the repo's isolated environment.
pub fn execute_shell_script(repo: &TestRepo, shell: &str, script: &str) -> String {
    // Use dev-detach wrapper to fully isolate shell from controlling terminals.
    // The dev-detach binary calls setsid() before exec'ing the shell, preventing
    // PTY-related hangs in nextest environments (unbuffer, script, terminal emulators).
    let detach = get_dev_detach_bin();
    let mut cmd = Command::new(detach);
    repo.clean_cli_env(&mut cmd);

    // Prevent user shell config from leaking into tests.
    cmd.env_remove("BASH_ENV");
    cmd.env_remove("ENV");
    cmd.env_remove("ZDOTDIR");
    cmd.env_remove("XONSHRC");
    cmd.env_remove("XDG_CONFIG_HOME");

    // Build argument list: dev-detach <shell> [shell-flags...] -c <script>
    let binary = get_shell_binary(shell);
    cmd.arg(binary);

    // Add shell-specific no-config flags
    match shell {
        "bash" => {
            cmd.arg("--noprofile").arg("--norc");
        }
        "zsh" => {
            cmd.arg("--no-globalrcs").arg("-f");
        }
        "fish" => {
            cmd.arg("--no-config");
        }
        "powershell" | "pwsh" => {
            cmd.arg("-NoProfile");
        }
        "xonsh" => {
            cmd.arg("--no-rc");
        }
        "nushell" | "nu" => {
            cmd.arg("--no-config-file");
        }
        _ => {}
    }

    cmd.arg("-c").arg(script);

    // Null stdin, piped stdout/stderr for full TTY isolation.
    // Combined with setsid() from dev-detach, this guarantees no controlling terminal.
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    // Execute with retry logic to handle race conditions where cargo temporarily
    // replaces the dev-detach binary during parallel test execution
    let output = match cmd.current_dir(repo.root_path()).output() {
        Ok(output) => output,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Binary temporarily unavailable - wait and retry once
            eprintln!("dev-detach spawn failed: {} - retrying after 100ms", e);
            std::thread::sleep(std::time::Duration::from_millis(100));

            // Rebuild the command for retry
            let detach = get_dev_detach_bin();
            let mut cmd = Command::new(detach);
            repo.clean_cli_env(&mut cmd);
            cmd.env_remove("BASH_ENV");
            cmd.env_remove("ENV");
            cmd.env_remove("ZDOTDIR");
            cmd.env_remove("XONSHRC");
            cmd.env_remove("XDG_CONFIG_HOME");
            cmd.arg(get_shell_binary(shell));
            match shell {
                "bash" => {
                    cmd.arg("--noprofile").arg("--norc");
                }
                "zsh" => {
                    cmd.arg("--no-globalrcs").arg("-f");
                }
                "fish" => {
                    cmd.arg("--no-config");
                }
                "powershell" | "pwsh" => {
                    cmd.arg("-NoProfile");
                }
                "xonsh" => {
                    cmd.arg("--no-rc");
                }
                "nushell" | "nu" => {
                    cmd.arg("--no-config-file");
                }
                _ => {}
            }
            cmd.arg("-c").arg(script);
            cmd.stdin(std::process::Stdio::null());
            cmd.stdout(std::process::Stdio::piped());
            cmd.stderr(std::process::Stdio::piped());

            cmd.current_dir(repo.root_path())
                .output()
                .unwrap_or_else(|e2| {
                    panic!("Failed to execute {} script after retry: {}", shell, e2)
                })
        }
        Err(e) => {
            panic!("Failed to execute {} script: {}", shell, e);
        }
    };

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Check for dev-detach-specific errors (setsid failures, execvp failures, etc.)
    if stderr.contains("dev-detach:") {
        panic!(
            "dev-detach binary error:\nstderr: {}\nstdout: {}",
            stderr,
            String::from_utf8_lossy(&output.stdout)
        );
    }

    if !output.status.success() {
        // Distinguish between normal exit and signal termination
        let exit_info = match output.status.code() {
            Some(code) => format!("exit code {}", code),
            None => "killed by signal".to_string(),
        };
        panic!(
            "Shell script failed ({}):\nCommand: dev-detach {} [shell-flags...] -c <script>\nstdout: {}\nstderr: {}",
            exit_info,
            shell,
            String::from_utf8_lossy(&output.stdout),
            stderr
        );
    }

    // Check for shell errors in stderr (command not found, syntax errors, etc.)
    // These indicate problems with our shell integration code
    if stderr.contains("command not found") || stderr.contains("not defined") {
        panic!(
            "Shell integration error detected:\nstderr: {}\nstdout: {}",
            stderr,
            String::from_utf8_lossy(&output.stdout)
        );
    }

    String::from_utf8(output.stdout).expect("Invalid UTF-8 in output")
}

/// Generate `wt init <shell>` output for the repo.
pub fn generate_init_code(repo: &TestRepo, shell: &str) -> String {
    let mut cmd = wt_command();
    repo.clean_cli_env(&mut cmd);

    let output = cmd
        .args(["init", shell])
        .current_dir(repo.root_path())
        .output()
        .expect("Failed to generate init code");

    let stdout = String::from_utf8(output.stdout).expect("Invalid UTF-8 in init code");
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() && stdout.trim().is_empty() {
        panic!("Failed to generate init code:\nstderr: {}", stderr);
    }

    // Check for shell errors in the generated init code when it's evaluated
    // This catches issues like missing compdef guards
    if stderr.contains("command not found") || stderr.contains("not defined") {
        panic!(
            "Init code contains errors:\nstderr: {}\nGenerated code:\n{}",
            stderr, stdout
        );
    }

    stdout
}

/// Format PATH mutation per shell.
pub fn path_export_syntax(shell: &str, bin_path: &str) -> String {
    match shell {
        "fish" => format!(r#"set -x PATH {} $PATH"#, bin_path),
        "nushell" => format!(r#"$env.PATH = ($env.PATH | prepend "{}")"#, bin_path),
        "powershell" => format!(r#"$env:PATH = "{}:$env:PATH""#, bin_path),
        "elvish" => format!(r#"set E:PATH = {}:$E:PATH"#, bin_path),
        "xonsh" => format!(r#"$PATH.insert(0, "{}")"#, bin_path),
        _ => format!(r#"export PATH="{}:$PATH""#, bin_path),
    }
}

/// Helper that returns the `wt` binary directory for PATH injection.
pub fn wt_bin_dir() -> String {
    get_cargo_bin("wt")
        .parent()
        .unwrap()
        .to_string_lossy()
        .to_string()
}
