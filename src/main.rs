use clap::Parser;
use move_clippy::cli::{Args, Command, LintArgs, OutputFormat};
use move_clippy::lint::LintRegistry;
use move_clippy::LintEngine;
use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(code) => code,
        Err(err) => {
            eprintln!("{err:#}");
            ExitCode::from(2)
        }
    }
}

fn run() -> anyhow::Result<ExitCode> {
    let args = Args::parse();

    match args.command {
        Some(Command::ListRules) => {
            list_rules();
            Ok(ExitCode::SUCCESS)
        }
        Some(Command::Explain { rule }) => {
            explain_rule(&rule)?;
            Ok(ExitCode::SUCCESS)
        }
        Some(Command::Lint(lint)) => lint_command(lint),
        None => lint_command(args.lint),
    }
}

fn list_rules() {
    let registry = LintRegistry::default_rules();
    let mut rules: Vec<_> = registry.descriptors().collect();
    rules.sort_by_key(|d| d.name);

    for d in rules {
        println!("{}\t{}\t{}", d.name, d.category.as_str(), d.description);
    }
}

fn explain_rule(rule: &str) -> anyhow::Result<()> {
    let registry = LintRegistry::default_rules();
    let Some(d) = registry.find_descriptor(rule) else {
        anyhow::bail!("unknown lint: {rule}");
    };

    println!("name: {}", d.name);
    println!("category: {}", d.category.as_str());
    println!("description: {}", d.description);
    Ok(())
}

fn lint_command(args: LintArgs) -> anyhow::Result<ExitCode> {
    let registry = LintRegistry::default_rules_filtered(&args.only, &args.skip)?;
    let engine = LintEngine::new(registry);

    let mut total_diags = 0usize;

    match args.format {
        OutputFormat::Json => {
            let mut out: Vec<JsonDiagnostic> = Vec::new();

            if args.paths.is_empty() {
                let (count, mut diags) = lint_stdin_json(&engine)?;
                total_diags += count;
                out.append(&mut diags);
            } else {
                let files = collect_move_files(&args.paths)?;
                for path in files {
                    let (count, mut diags) = lint_file_json(&engine, &path)?;
                    total_diags += count;
                    out.append(&mut diags);
                }
            }

            out.sort_by(|a, b| {
                (a.file.as_str(), a.row, a.column, a.lint.as_str())
                    .cmp(&(b.file.as_str(), b.row, b.column, b.lint.as_str()))
            });

            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        OutputFormat::Pretty | OutputFormat::Github => {
            if args.paths.is_empty() {
                total_diags += lint_stdin_text(&engine, args.format)?;
            } else {
                let files = collect_move_files(&args.paths)?;
                for path in files {
                    total_diags += lint_file_text(&engine, &path, args.format)?;
                }
            }
        }
    }

    if args.deny_warnings && total_diags > 0 {
        Ok(ExitCode::from(1))
    } else {
        Ok(ExitCode::SUCCESS)
    }
}

#[derive(Debug, Serialize)]
struct JsonDiagnostic {
    file: String,
    row: usize,
    column: usize,
    lint: String,
    message: String,
}

fn lint_file_text(engine: &LintEngine, path: &Path, format: OutputFormat) -> anyhow::Result<usize> {
    let source = std::fs::read_to_string(path)?;
    let diagnostics = engine.lint_source(&source)?;

    match format {
        OutputFormat::Pretty => {
            for diag in &diagnostics {
                println!(
                    "{}:{}:{}: {}: {}",
                    path.display(),
                    diag.span.start.row,
                    diag.span.start.column,
                    diag.lint.name,
                    diag.message
                );
            }
            println!("{} diagnostics for {}", diagnostics.len(), path.display());
        }
        OutputFormat::Github => {
            for diag in &diagnostics {
                let msg = github_escape(&diag.message);
                println!(
                    "::warning file={},line={},col={},title={}::{}",
                    path.display(),
                    diag.span.start.row,
                    diag.span.start.column,
                    diag.lint.name,
                    msg
                );
            }
        }
        OutputFormat::Json => unreachable!("json handled elsewhere"),
    }

    Ok(diagnostics.len())
}

fn lint_stdin_text(engine: &LintEngine, format: OutputFormat) -> anyhow::Result<usize> {
    let mut source = String::new();
    std::io::stdin().read_to_string(&mut source)?;
    let diagnostics = engine.lint_source(&source)?;

    match format {
        OutputFormat::Pretty => {
            for diag in &diagnostics {
                println!(
                    "stdin:{}:{}: {}: {}",
                    diag.span.start.row,
                    diag.span.start.column,
                    diag.lint.name,
                    diag.message
                );
            }
            println!("{} diagnostics for stdin", diagnostics.len());
        }
        OutputFormat::Github => {
            for diag in &diagnostics {
                let msg = github_escape(&diag.message);
                println!(
                    "::warning file=stdin,line={},col={},title={}::{}",
                    diag.span.start.row,
                    diag.span.start.column,
                    diag.lint.name,
                    msg
                );
            }
        }
        OutputFormat::Json => unreachable!("json handled elsewhere"),
    }

    Ok(diagnostics.len())
}

fn lint_file_json(engine: &LintEngine, path: &Path) -> anyhow::Result<(usize, Vec<JsonDiagnostic>)> {
    let source = std::fs::read_to_string(path)?;
    let diagnostics = engine.lint_source(&source)?;
    let file = path.display().to_string();

    let out = diagnostics
        .iter()
        .map(|d| JsonDiagnostic {
            file: file.clone(),
            row: d.span.start.row,
            column: d.span.start.column,
            lint: d.lint.name.to_string(),
            message: d.message.clone(),
        })
        .collect::<Vec<_>>();

    Ok((diagnostics.len(), out))
}

fn lint_stdin_json(engine: &LintEngine) -> anyhow::Result<(usize, Vec<JsonDiagnostic>)> {
    let mut source = String::new();
    std::io::stdin().read_to_string(&mut source)?;
    let diagnostics = engine.lint_source(&source)?;

    let out = diagnostics
        .iter()
        .map(|d| JsonDiagnostic {
            file: "stdin".to_string(),
            row: d.span.start.row,
            column: d.span.start.column,
            lint: d.lint.name.to_string(),
            message: d.message.clone(),
        })
        .collect::<Vec<_>>();

    Ok((diagnostics.len(), out))
}

fn github_escape(s: &str) -> String {
    s.replace('%', "%25")
        .replace('\r', "%0D")
        .replace('\n', "%0A")
}

fn collect_move_files(paths: &[PathBuf]) -> anyhow::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for path in paths {
        collect_from_path(path, &mut out)?;
    }

    out.sort();
    out.dedup();
    Ok(out)
}

fn collect_from_path(path: &Path, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    let meta = std::fs::metadata(path)?;
    if meta.is_dir() {
        collect_from_dir(path, out)
    } else {
        out.push(path.to_path_buf());
        Ok(())
    }
}

fn collect_from_dir(dir: &Path, out: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            if should_skip_dir(&path) {
                continue;
            }
            collect_from_dir(&path, out)?;
            continue;
        }

        if path.extension().and_then(|e| e.to_str()) == Some("move") {
            out.push(path);
        }
    }

    Ok(())
}

fn should_skip_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
        return false;
    };

    matches!(name, ".git" | "target" | "build")
}
