use clap::{Parser, ValueEnum};

#[derive(ValueEnum, Clone)]
pub enum CompletionShell {
    Bash, Zsh, Fish, Powershell, Elvish, Nushell,
}

#[derive(Parser)]
#[command(author, version, about = "JSON Query Merge for OpenCode", long_about = None)]
pub struct Cli {
    /// The path to the config file (defaults to ~/.config/opencode/opencode.jsonc)
    #[arg(short = 'c', long)]
    pub config_path: Option<String>,

    /// Interactively select keys or key->value pairs to add
    #[arg(short, long)]
    pub interactive: bool,

    /// Suppress the auto-fix prompt for missing JSON braces
    #[arg(long)]
    pub no_autofix: bool,

    /// Provide a custom map (format: oldKey=newKey,o2=n2)
    #[arg(short = 'm',long)]
    pub map: Option<String>,

    /// Save the provided --map to the config file for future use
    #[arg(long, requires = "map")]
    pub save_map: bool,

    /// Set and save a custom map, overwriting the existing config
    #[arg(long, conflicts_with_all = ["map", "reset_map", "no_custom_map"])]
    pub set_map: Option<String>,

    /// Ignore any saved custom map in the config file
    #[arg(long, conflicts_with_all = ["map", "set_map", "reset_map"])]
    pub no_custom_map: bool,

    /// Clear/reset the saved custom map in the config file
    #[arg(long, conflicts_with_all = ["map", "set_map", "no_custom_map"])]
    pub reset_map: bool,

    /// Generate shell completion scripts
    #[arg(long, value_enum, hide = true)]
    pub generate_completions: Option<CompletionShell>,

    /// Path to a custom schema (local file or remote URL)
    #[arg(short = 's',long)]
    pub schema_path: Option<String>,

    /// Register an alias in the current shell with current flags
    #[arg(long)]
    pub register_alias: Option<String>,

    /// Dry run: perform logic but do not write to disk
    #[arg(short = 'd',long)]
    pub dry_run: bool,
}
