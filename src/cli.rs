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

    /// Triage findings - track, categorize, and report lint results.
    Triage(TriageCommand),
}

// ============================================================================
// Triage Subcommand
// ============================================================================

#[derive(Debug, Clone, ClapArgs)]
pub struct TriageCommand {
    #[command(subcommand)]
    pub action: TriageAction,

    /// Path to triage database file.
    #[arg(long, default_value = "triage.json", global = true)]
    pub database: PathBuf,
}

#[derive(Debug, Clone, Subcommand)]
pub enum TriageAction {
    /// List findings with optional filtering.
    List {
        /// Filter by status (needs_review, confirmed, false_positive, wont_fix).
        #[arg(long)]
        status: Option<String>,

        /// Filter by lint name.
        #[arg(long)]
        lint: Option<String>,

        /// Filter by repository.
        #[arg(long)]
        repo: Option<String>,

        /// Filter by severity (critical, high, medium, low, info).
        #[arg(long)]
        severity: Option<String>,

        /// Filter by category (security, style, etc.).
        #[arg(long)]
        category: Option<String>,

        /// Maximum number of results to show.
        #[arg(long, default_value = "50")]
        limit: usize,
    },

    /// Show detailed info about a specific finding.
    Show {
        /// Finding ID (or prefix).
        id: String,
    },

    /// Update the status of a finding.
    Update {
        /// Finding ID (or prefix).
        id: String,

        /// New status (confirmed, false_positive, wont_fix, needs_review).
        #[arg(long)]
        status: String,

        /// Optional notes about this finding.
        #[arg(long)]
        notes: Option<String>,
    },

    /// Generate a summary report.
    Report {
        /// Output format (md, json, text).
        #[arg(long, default_value = "md")]
        format: String,

        /// Group findings by field (lint, repo, status, severity).
        #[arg(long)]
        group_by: Option<String>,

        /// Output file (stdout if not specified).
        #[arg(long, short)]
        output: Option<PathBuf>,
    },

    /// Import findings from lint output (JSON format).
    Import {
        /// Path to JSON file with lint output.
        input: PathBuf,

        /// Repository name to associate with findings.
        #[arg(long)]
        repo: String,
    },

    /// Show summary statistics.
    Summary,

    /// Show detailed statistics by lint, repo, or category.
    Stats {
        /// Group by field: lint, repo, category, severity.
        #[arg(long, default_value = "lint")]
        by: String,

        /// Only show entries with this minimum count.
        #[arg(long, default_value = "1")]
        min_count: usize,

        /// Sort by: total, confirmed, fp, fp_rate.
        #[arg(long, default_value = "total")]
        sort: String,
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

    /// Apply safe auto-fixes to files.
    ///
    /// Only machine-applicable fixes are applied by default.
    /// Use --unsafe-fixes to also apply potentially unsafe fixes.
    #[arg(long)]
    pub fix: bool,

    /// Preview fixes without applying them (requires --fix).
    ///
    /// Shows a unified diff of what changes would be made.
    #[arg(long, requires = "fix")]
    pub fix_dry_run: bool,

    /// Apply unsafe fixes (requires --fix).
    ///
    /// Unsafe fixes may change runtime behavior. Review changes carefully
    /// before committing.
    #[arg(long, requires = "fix")]
    pub unsafe_fixes: bool,

    /// Skip creating .bak backup files before applying fixes (requires --fix).
    ///
    /// By default, move-clippy creates a .bak backup of each file before
    /// modifying it. Use this flag to disable backups.
    #[arg(long, requires = "fix")]
    pub no_backup: bool,
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
