use clap::Parser;
use move_clippy::LintEngine;
use move_clippy::cli::{Args, Command, LintArgs, LintMode, OutputFormat};
use move_clippy::config;
use move_clippy::level::LintLevel;
use move_clippy::lint::{LintRegistry, LintSettings, is_semantic_lint};
use move_clippy::semantic;
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
    rules.extend(semantic::descriptors());
    rules.sort_by_key(|d| d.name);

    for d in rules {
        let fix_status = if d.fix.available {
            format!(" [fix: {}]", d.fix.safety.as_str())
        } else {
            String::new()
        };
        println!(
            "{}\t{}\t{}\t{}{}",
            d.name,
            d.category.as_str(),
            d.group.as_str(),
            d.description,
            fix_status
        );
    }
}

fn explain_rule(rule: &str) -> anyhow::Result<()> {
    let registry = LintRegistry::default_rules();
    let d = registry
        .find_descriptor(rule)
        .or_else(|| semantic::find_descriptor(rule));
    let Some(d) = d else {
        anyhow::bail!("unknown lint: {rule}");
    };

    println!("name: {}", d.name);
    println!("category: {}", d.category.as_str());
    println!("group: {}", d.group.as_str());
    println!("description: {}", d.description);
    if d.fix.available {
        println!("fix: available ({})", d.fix.safety.as_str());
        if !d.fix.description.is_empty() {
            println!("fix description: {}", d.fix.description);
        }
    } else {
        println!("fix: not available");
    }
    Ok(())
}

fn lint_command(args: LintArgs) -> anyhow::Result<ExitCode> {
    let start_dir = infer_start_dir(&args)?;
    let loaded_cfg = config::load_config(args.config.as_deref(), &start_dir)?;

    let (disabled, settings, preview) = match loaded_cfg.as_ref() {
        Some((_path, cfg)) => (
            cfg.lints.disabled.clone(),
            LintSettings::default()
                .with_config_levels(cfg.lints.levels.clone())
                .disable(cfg.lints.disabled.clone()),
            // CLI flag takes precedence over config
            args.preview || cfg.lints.preview,
        ),
        None => (Vec::new(), LintSettings::default(), args.preview),
    };

    if matches!(args.mode, LintMode::Fast) && args.only.iter().any(|n| is_semantic_lint(n.as_str()))
    {
        anyhow::bail!("semantic lints require --mode full");
    }

    let semantic_diags = if matches!(args.mode, LintMode::Full) {
        let semantic_selected = if args.only.is_empty() {
            true
        } else {
            args.only.iter().any(|n| is_semantic_lint(n.as_str()))
        };

        if !semantic_selected {
            Vec::new()
        } else {
            let Some(pkg_hint) = args
                .package
                .as_deref()
                .or_else(|| args.paths.first().map(|p| p.as_path()))
            else {
                anyhow::bail!("--mode full requires either --package or at least one PATH");
            };

            let mut diags = semantic::lint_package(pkg_hint, &settings)?;

            if !args.only.is_empty() {
                let only_set: std::collections::HashSet<&str> =
                    args.only.iter().map(|s| s.as_str()).collect();
                diags.retain(|d| only_set.contains(d.lint.name));
            }

            if !args.skip.is_empty() {
                let skip_set: std::collections::HashSet<&str> =
                    args.skip.iter().map(|s| s.as_str()).collect();
                diags.retain(|d| !skip_set.contains(d.lint.name));
            }

            diags
        }
    } else {
        Vec::new()
    };

    let registry = LintRegistry::default_rules_filtered(
        &args.only,
        &args.skip,
        &disabled,
        matches!(args.mode, LintMode::Full),
        preview,
    )?;
    let engine = LintEngine::new_with_settings(registry, settings.clone());

    let mut total_diags = 0usize;
    let mut has_error = false;

    match args.format {
        OutputFormat::Json => {
            let mut out: Vec<JsonDiagnostic> = Vec::new();

            if args.paths.is_empty() {
                let (count, file_has_error, mut diags) = lint_stdin_json(&engine)?;
                total_diags += count;
                has_error |= file_has_error;
                out.append(&mut diags);
            } else {
                let files = collect_move_files(&args.paths)?;
                for path in files {
                    let (count, file_has_error, mut diags) = lint_file_json(&engine, &path)?;
                    total_diags += count;
                    has_error |= file_has_error;
                    out.append(&mut diags);
                }
            }

            if !semantic_diags.is_empty() {
                for d in &semantic_diags {
                    let file = d.file.clone().unwrap_or_else(|| "<unknown>".to_string());
                    has_error |= d.level == LintLevel::Error;
                    total_diags += 1;
                    out.push(JsonDiagnostic {
                        file,
                        row: d.span.start.row,
                        column: d.span.start.column,
                        level: d.level.as_str().to_string(),
                        lint: d.lint.name.to_string(),
                        message: d.message.clone(),
                    });
                }
            }

            out.sort_by(|a, b| {
                (
                    a.file.as_str(),
                    a.row,
                    a.column,
                    a.level.as_str(),
                    a.lint.as_str(),
                )
                    .cmp(&(
                        b.file.as_str(),
                        b.row,
                        b.column,
                        b.level.as_str(),
                        b.lint.as_str(),
                    ))
            });

            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        OutputFormat::Pretty | OutputFormat::Github => {
            if args.paths.is_empty() {
                let (count, file_has_error) =
                    lint_stdin_text(&engine, args.format, args.deny_warnings)?;
                total_diags += count;
                has_error |= file_has_error;
            } else {
                let files = collect_move_files(&args.paths)?;
                for path in files {
                    let (count, file_has_error) =
                        lint_file_text(&engine, &path, args.format, args.deny_warnings)?;
                    total_diags += count;
                    has_error |= file_has_error;
                }
            }

            if !semantic_diags.is_empty() {
                for diag in &semantic_diags {
                    let file = diag.file.clone().unwrap_or_else(|| "<unknown>".to_string());
                    match args.format {
                        OutputFormat::Pretty => {
                            println!(
                                "{}:{}:{}: {}: {}: {}",
                                file,
                                diag.span.start.row,
                                diag.span.start.column,
                                diag.level.as_str(),
                                diag.lint.name,
                                diag.message
                            );
                        }
                        OutputFormat::Github => {
                            let msg = github_escape(&diag.message);
                            let kind = if diag.level == LintLevel::Error
                                || (args.deny_warnings && diag.level == LintLevel::Warn)
                            {
                                "error"
                            } else {
                                "warning"
                            };
                            println!(
                                "::{} file={},line={},col={},title={}::{}",
                                kind,
                                github_escape(&file),
                                diag.span.start.row,
                                diag.span.start.column,
                                diag.lint.name,
                                msg
                            );
                        }
                        OutputFormat::Json => unreachable!(),
                    }

                    has_error |= diag.level == LintLevel::Error;
                    total_diags += 1;
                }
            }
        }
    }

    if has_error || (args.deny_warnings && total_diags > 0) {
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
    level: String,
    lint: String,
    message: String,
}

fn lint_file_text(
    engine: &LintEngine,
    path: &Path,
    format: OutputFormat,
    deny_warnings: bool,
) -> anyhow::Result<(usize, bool)> {
    let source = std::fs::read_to_string(path)?;
    let diagnostics = engine.lint_source(&source)?;

    let mut has_error = false;

    match format {
        OutputFormat::Pretty => {
            for diag in &diagnostics {
                let file = diag
                    .file
                    .clone()
                    .unwrap_or_else(|| path.display().to_string());
                println!(
                    "{}:{}:{}: {}: {}: {}",
                    file,
                    diag.span.start.row,
                    diag.span.start.column,
                    diag.level.as_str(),
                    diag.lint.name,
                    diag.message
                );
                has_error |= diag.level == LintLevel::Error;
            }
            println!("{} diagnostics for {}", diagnostics.len(), path.display());
        }
        OutputFormat::Github => {
            for diag in &diagnostics {
                let file = diag
                    .file
                    .clone()
                    .unwrap_or_else(|| path.display().to_string());
                let msg = github_escape(&diag.message);

                let kind = if diag.level == LintLevel::Error
                    || (deny_warnings && diag.level == LintLevel::Warn)
                {
                    "error"
                } else {
                    "warning"
                };

                println!(
                    "::{} file={},line={},col={},title={}::{}",
                    kind,
                    github_escape(&file),
                    diag.span.start.row,
                    diag.span.start.column,
                    diag.lint.name,
                    msg
                );
                has_error |= kind == "error";
            }
        }
        OutputFormat::Json => unreachable!("json handled elsewhere"),
    }

    Ok((diagnostics.len(), has_error))
}

fn lint_stdin_text(
    engine: &LintEngine,
    format: OutputFormat,
    deny_warnings: bool,
) -> anyhow::Result<(usize, bool)> {
    let mut source = String::new();
    std::io::stdin().read_to_string(&mut source)?;
    let diagnostics = engine.lint_source(&source)?;

    let mut has_error = false;

    match format {
        OutputFormat::Pretty => {
            for diag in &diagnostics {
                let file = diag.file.clone().unwrap_or_else(|| "stdin".to_string());
                println!(
                    "{}:{}:{}: {}: {}: {}",
                    file,
                    diag.span.start.row,
                    diag.span.start.column,
                    diag.level.as_str(),
                    diag.lint.name,
                    diag.message
                );
                has_error |= diag.level == LintLevel::Error;
            }
            println!("{} diagnostics for stdin", diagnostics.len());
        }
        OutputFormat::Github => {
            for diag in &diagnostics {
                let file = diag.file.clone().unwrap_or_else(|| "stdin".to_string());
                let msg = github_escape(&diag.message);

                let kind = if diag.level == LintLevel::Error
                    || (deny_warnings && diag.level == LintLevel::Warn)
                {
                    "error"
                } else {
                    "warning"
                };

                println!(
                    "::{} file={},line={},col={},title={}::{}",
                    kind,
                    github_escape(&file),
                    diag.span.start.row,
                    diag.span.start.column,
                    diag.lint.name,
                    msg
                );
                has_error |= kind == "error";
            }
        }
        OutputFormat::Json => unreachable!("json handled elsewhere"),
    }

    Ok((diagnostics.len(), has_error))
}

fn lint_file_json(
    engine: &LintEngine,
    path: &Path,
) -> anyhow::Result<(usize, bool, Vec<JsonDiagnostic>)> {
    let source = std::fs::read_to_string(path)?;
    let diagnostics = engine.lint_source(&source)?;

    let mut has_error = false;

    let out = diagnostics
        .iter()
        .map(|d| {
            let file = d.file.clone().unwrap_or_else(|| path.display().to_string());
            has_error |= d.level == LintLevel::Error;
            JsonDiagnostic {
                file,
                row: d.span.start.row,
                column: d.span.start.column,
                level: d.level.as_str().to_string(),
                lint: d.lint.name.to_string(),
                message: d.message.clone(),
            }
        })
        .collect::<Vec<_>>();

    Ok((diagnostics.len(), has_error, out))
}

fn lint_stdin_json(engine: &LintEngine) -> anyhow::Result<(usize, bool, Vec<JsonDiagnostic>)> {
    let mut source = String::new();
    std::io::stdin().read_to_string(&mut source)?;
    let diagnostics = engine.lint_source(&source)?;

    let mut has_error = false;

    let out = diagnostics
        .iter()
        .map(|d| {
            let file = d.file.clone().unwrap_or_else(|| "stdin".to_string());
            has_error |= d.level == LintLevel::Error;
            JsonDiagnostic {
                file,
                row: d.span.start.row,
                column: d.span.start.column,
                level: d.level.as_str().to_string(),
                lint: d.lint.name.to_string(),
                message: d.message.clone(),
            }
        })
        .collect::<Vec<_>>();

    Ok((diagnostics.len(), has_error, out))
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

fn infer_start_dir(args: &LintArgs) -> anyhow::Result<PathBuf> {
    let base = if let Some(pkg) = &args.package {
        pkg.clone()
    } else if let Some(p) = args.paths.first() {
        p.clone()
    } else {
        std::env::current_dir()?
    };

    let base = if base.is_file() {
        base.parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
    } else {
        base
    };

    Ok(base)
}
