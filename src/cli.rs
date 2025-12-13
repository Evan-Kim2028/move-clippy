use clap::{Args as ClapArgs, Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Move Clippy CLI options.
#[derive(Debug, Parser)]
#[command(
    name = "move-clippy",
    version,
    about = "Lint Move code for style and modernization",
    args_conflicts_with_subcommands = true,
    subcommand_precedence_over_arg = true
)]
pub struct Args {
    #[command(subcommand)]
    pub command: Option<Command>,

    #[command(flatten)]
    pub lint: LintArgs,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Lint files or directories.
    Lint(LintArgs),

    /// List available lints.
    ListRules,

    /// Explain a lint.
    Explain {
        /// Lint rule name.
        rule: String,
    },
}

#[derive(Debug, Clone, ClapArgs)]
pub struct LintArgs {
    /// Files/directories to lint. Defaults to stdin when absent.
    #[arg(value_name = "PATH")]
    pub paths: Vec<PathBuf>,

    /// Output format.
    #[arg(long, value_enum, default_value_t = OutputFormat::Pretty)]
    pub format: OutputFormat,

    /// Only run these lints (comma-separated).
    #[arg(long, value_delimiter = ',')]
    pub only: Vec<String>,

    /// Skip these lints (comma-separated).
    #[arg(long, value_delimiter = ',')]
    pub skip: Vec<String>,

    /// Exit with code 1 if any diagnostics are emitted.
    #[arg(long)]
    pub deny_warnings: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    Pretty,
    Json,
    Github,
}
