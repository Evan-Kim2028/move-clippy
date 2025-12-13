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

    /// Lint mode.
    #[arg(long, value_enum, default_value_t = LintMode::Fast)]
    pub mode: LintMode,

    /// Treat the inputs as belonging to a Move package and use this path as the package root.
    ///
    /// If omitted, full mode will attempt to infer a package root from the first PATH.
    #[arg(long, value_name = "PATH")]
    pub package: Option<PathBuf>,

    /// Path to a move-clippy.toml config file. If omitted, move-clippy searches parent directories.
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,

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

    /// Enable preview rules that are not yet stable.
    ///
    /// Preview rules may have higher false-positive rates or change behavior
    /// between versions. Use with caution.
    #[arg(long)]
    pub preview: bool,

    /// Apply unsafe fixes (requires --fix when implemented).
    ///
    /// Unsafe fixes may change runtime behavior. Review changes carefully
    /// before committing.
    #[arg(long)]
    pub unsafe_fixes: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum LintMode {
    Fast,
    Full,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    Pretty,
    Json,
    Github,
}
