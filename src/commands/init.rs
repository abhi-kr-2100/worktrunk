use clap::Command;
use clap_complete::{Shell as CompletionShell, generate};
use worktrunk::shell;

pub fn handle_init(shell_name: &str, cmd_name: &str, cli_cmd: &mut Command) -> Result<(), String> {
    let shell = shell_name.parse::<shell::Shell>()?;

    let init = shell::ShellInit::new(shell, cmd_name.to_string());

    // Generate shell integration code
    let integration_output = init
        .generate()
        .map_err(|e| format!("Failed to generate shell code: {}", e))?;

    println!("{}", integration_output);

    // Generate and append static completions
    println!();
    println!("# Static completions (commands and flags)");

    // Check if shell supports completion
    if !shell.supports_completion() {
        eprintln!("Completion not yet supported for {}", shell);
        std::process::exit(1);
    }

    // Generate completions to a string so we can filter out hidden commands
    let mut completion_output = Vec::new();
    let completion_shell = match shell {
        shell::Shell::Bash | shell::Shell::Oil => CompletionShell::Bash,
        shell::Shell::Fish => CompletionShell::Fish,
        shell::Shell::Zsh => CompletionShell::Zsh,
        _ => unreachable!(
            "supports_completion() check above ensures we only reach this for supported shells"
        ),
    };
    generate(completion_shell, cli_cmd, "wt", &mut completion_output);

    // Filter out lines for hidden commands (completion, complete)
    let completion_str = String::from_utf8_lossy(&completion_output);
    let filtered: Vec<&str> = completion_str
        .lines()
        .filter(|line| {
            // Remove lines that complete the hidden commands
            !(line.contains("\"completion\"")
                || line.contains("\"complete\"")
                || line.contains("-a \"completion\"")
                || line.contains("-a \"complete\""))
        })
        .collect();

    for line in filtered {
        println!("{}", line);
    }

    Ok(())
}
