use clap::Parser;
use move_clippy::LintEngine;
use move_clippy::cli::{
    Args, Command, LintArgs, LintMode, OutputFormat, TriageAction, TriageCommand,
};
use move_clippy::config;
use move_clippy::fixer;
use move_clippy::level::LintLevel;
use move_clippy::lint::{LintRegistry, LintSettings, resolve_lint_alias};
use move_clippy::semantic;
use move_clippy::triage::{
    Finding, FindingFilter, ReportFormat, Severity, TriageDatabase, TriageStatus,
    generate_json_report, generate_markdown_report, generate_text_report,
};
use move_clippy::unified::{self, LintPhase};
use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    move_clippy::telemetry::init_tracing();
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
        Some(Command::Triage(triage)) => triage_command(triage),
        None => lint_command(args.lint),
    }
}

fn list_rules() {
    let registry = unified::unified_registry();
    let mut rules: Vec<_> = registry.descriptors().collect();
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
    let canonical = resolve_lint_alias(rule);
    let Some(lint) = unified::unified_registry().get(canonical) else {
        anyhow::bail!("unknown lint: {rule}");
    };
    let d = lint.descriptor;

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
    // Handle --fix mode
    if args.fix {
        return fix_command(args);
    }

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

    let only_requires_full = args.only.iter().any(|n| {
        unified::lint_phase(resolve_lint_alias(n.as_str()))
            .is_some_and(|phase| phase != LintPhase::Syntactic)
    });

    if matches!(args.mode, LintMode::Fast) && only_requires_full {
        anyhow::bail!("semantic lints require --mode full");
    }

    let semantic_diags = if matches!(args.mode, LintMode::Full) {
        let semantic_selected = args.only.is_empty() || only_requires_full;

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

            let mut diags =
                semantic::lint_package(pkg_hint, &settings, preview, args.experimental)?;

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

    let registry = LintRegistry::default_rules_filtered_with_experimental(
        &args.only,
        &args.skip,
        &disabled,
        matches!(args.mode, LintMode::Full),
        preview,
        args.experimental,
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
                    lint_stdin_text(&engine, args.format, args.deny_warnings, args.show_tier)?;
                total_diags += count;
                has_error |= file_has_error;
            } else {
                let files = collect_move_files(&args.paths)?;
                for path in files {
                    let (count, file_has_error) = lint_file_text(
                        &engine,
                        &path,
                        args.format,
                        args.deny_warnings,
                        args.show_tier,
                    )?;
                    total_diags += count;
                    has_error |= file_has_error;
                }
            }

            if !semantic_diags.is_empty() {
                for diag in &semantic_diags {
                    let file = diag.file.clone().unwrap_or_else(|| "<unknown>".to_string());
                    match args.format {
                        OutputFormat::Pretty => {
                            let tier_prefix = if args.show_tier {
                                format!("[{}] ", diag.lint.group.as_str())
                            } else {
                                String::new()
                            };
                            println!(
                                "{}:{}:{}: {}: {}{}: {}",
                                file,
                                diag.span.start.row,
                                diag.span.start.column,
                                diag.level.as_str(),
                                tier_prefix,
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
                            let lint_name = if args.show_tier {
                                format!("[{}]{}", diag.lint.group.as_str(), diag.lint.name)
                            } else {
                                diag.lint.name.to_string()
                            };
                            println!(
                                "::{} file={},line={},col={},title={}::{}",
                                kind,
                                github_escape(&file),
                                diag.span.start.row,
                                diag.span.start.column,
                                lint_name,
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

#[derive(Debug, Serialize, serde::Deserialize)]
struct JsonDiagnostic {
    file: String,
    row: usize,
    column: usize,
    level: String,
    lint: String,
    message: String,
}

/// Handle --fix mode: apply auto-fixes to files.
fn fix_command(args: LintArgs) -> anyhow::Result<ExitCode> {
    if args.paths.is_empty() {
        anyhow::bail!("--fix requires file paths (stdin not supported)");
    }

    let start_dir = infer_start_dir(&args)?;
    let loaded_cfg = config::load_config(args.config.as_deref(), &start_dir)?;

    let (disabled, settings, preview) = match loaded_cfg.as_ref() {
        Some((_path, cfg)) => (
            cfg.lints.disabled.clone(),
            LintSettings::default()
                .with_config_levels(cfg.lints.levels.clone())
                .disable(cfg.lints.disabled.clone()),
            args.preview || cfg.lints.preview,
        ),
        None => (Vec::new(), LintSettings::default(), args.preview),
    };

    let registry = LintRegistry::default_rules_filtered_with_experimental(
        &args.only,
        &args.skip,
        &disabled,
        matches!(args.mode, LintMode::Full),
        preview,
        args.experimental,
    )?;
    let engine = LintEngine::new_with_settings(registry, settings);

    let files = collect_move_files(&args.paths)?;
    let mut total_fixed = 0usize;
    let mut total_skipped = 0usize;
    let mut files_modified = 0usize;

    const MAX_ITERATIONS: usize = 10; // Prevent infinite loops

    for path in &files {
        let original_source = std::fs::read_to_string(path)?;
        let mut current_source = original_source.clone();
        let mut file_fixes = 0usize;
        let mut iterations = 0usize;

        // Iterate until no more fixes are applied (or max iterations reached)
        loop {
            iterations += 1;
            if iterations > MAX_ITERATIONS {
                eprintln!(
                    "Warning: Max fix iterations ({}) reached for {}",
                    MAX_ITERATIONS,
                    path.display()
                );
                break;
            }

            let diagnostics = engine.lint_source(&current_source)?;

            // Filter to diagnostics with fix suggestions
            let fixable: Vec<_> = diagnostics
                .iter()
                .filter(|d| d.suggestion.is_some())
                .cloned()
                .collect();

            if fixable.is_empty() {
                break;
            }

            let result = fixer::apply_fixes(&current_source, &fixable, args.unsafe_fixes)?;

            if result.fixes_applied == 0 {
                total_skipped += result.fixes_skipped;
                break;
            }

            file_fixes += result.fixes_applied;
            current_source = result.fixed_source;

            // In dry-run mode, only do one iteration
            if args.fix_dry_run {
                total_skipped += result.fixes_skipped;
                break;
            }
        }

        if file_fixes > 0 {
            if args.fix_dry_run {
                // Print diff
                let diff = fixer::format_diff(&original_source, &current_source, path);
                if !diff.is_empty() {
                    println!("{}", diff);
                }
            } else {
                // Create backup unless --no-backup is set
                if !args.no_backup {
                    let backup_path = path.with_extension(format!(
                        "{}.bak",
                        path.extension().unwrap_or_default().to_string_lossy()
                    ));
                    std::fs::write(&backup_path, &original_source)?;
                }

                // Write fixed source
                std::fs::write(path, &current_source)?;
                files_modified += 1;
            }
            total_fixed += file_fixes;
        }
    }

    // Print summary
    if args.fix_dry_run {
        println!(
            "\n{} fix(es) would be applied to {} file(s)",
            total_fixed,
            files.len()
        );
        if total_skipped > 0 {
            println!(
                "{} fix(es) skipped (use --unsafe-fixes to apply)",
                total_skipped
            );
        }
    } else {
        println!(
            "Applied {} fix(es) to {} file(s)",
            total_fixed, files_modified
        );
        if total_skipped > 0 {
            println!(
                "{} fix(es) skipped (use --unsafe-fixes to apply)",
                total_skipped
            );
        }
    }

    Ok(ExitCode::SUCCESS)
}

// ============================================================================
// Triage Command
// ============================================================================

/// Handle triage subcommand for tracking and categorizing findings.
fn triage_command(cmd: TriageCommand) -> anyhow::Result<ExitCode> {
    let db_path = &cmd.database;

    match cmd.action {
        TriageAction::List {
            status,
            lint,
            repo,
            severity,
            category,
            limit,
        } => {
            let db = TriageDatabase::load(db_path)?;

            // Build filter
            let mut filter = FindingFilter::new();

            if let Some(s) = status {
                filter = filter.with_status(TriageStatus::from_str(&s)?);
            }
            if let Some(l) = lint {
                filter = filter.with_lint(l);
            }
            if let Some(r) = repo {
                filter = filter.with_repo(r);
            }
            if let Some(sev) = severity
                && let Some(s) = Severity::from_str(&sev)
            {
                filter = filter.with_severity(s);
            }
            if let Some(c) = category {
                filter = filter.with_category(c);
            }

            let mut findings: Vec<_> = db.filter(&filter);
            findings.sort_by(|a, b| (&a.repo, &a.file, a.line).cmp(&(&b.repo, &b.file, b.line)));

            let total = findings.len();
            let shown = findings.iter().take(limit);

            println!("Found {} findings (showing up to {}):\n", total, limit);

            for finding in shown {
                println!("{}", finding.short_display());
            }

            if total > limit {
                println!(
                    "\n... and {} more (use --limit to show more)",
                    total - limit
                );
            }

            Ok(ExitCode::SUCCESS)
        }

        TriageAction::Show { id } => {
            let db = TriageDatabase::load(db_path)?;
            // Try exact match first, then prefix match
            let finding = db
                .get(&id)
                .or_else(|| db.list_all().into_iter().find(|f| f.id.starts_with(&id)));
            let Some(finding) = finding else {
                eprintln!("Finding not found: {}", id);
                return Ok(ExitCode::from(1));
            };
            println!("Finding: {}", finding.id);
            println!("Status:  {}", finding.status);
            println!("Lint:    {}", finding.lint);
            println!("Category: {}", finding.category);
            println!("Severity: {}", finding.severity);
            println!("Repo:    {}", finding.repo);
            println!("File:    {}:{}", finding.file, finding.line);

            // Display snippet with line numbers if available
            if let (Some(snippet), Some(start_line)) =
                (&finding.snippet, finding.snippet_start_line)
            {
                println!("\nCode:");
                let formatted = move_clippy::triage::format_snippet_with_lines(
                    snippet,
                    start_line,
                    finding.line,
                );
                print!("{}", formatted);
            }

            println!("\nMessage: {}", finding.message);

            if let Some(notes) = &finding.notes {
                println!("Notes:   {}", notes);
            }

            println!(
                "\nDetected: {}",
                finding.detected_at.format("%Y-%m-%d %H:%M UTC")
            );
            if let Some(reviewed) = finding.reviewed_at {
                println!("Reviewed: {}", reviewed.format("%Y-%m-%d %H:%M UTC"));
            }

            Ok(ExitCode::SUCCESS)
        }

        TriageAction::Update { id, status, notes } => {
            let mut db = TriageDatabase::load(db_path)?;

            // Find by exact or prefix match
            let finding_id = if db.get(&id).is_some() {
                id.clone()
            } else {
                db.list_all()
                    .into_iter()
                    .find(|f| f.id.starts_with(&id))
                    .map(|f| f.id.clone())
                    .ok_or_else(|| anyhow::anyhow!("Finding not found: {}", id))?
            };

            let new_status = TriageStatus::from_str(&status)?;
            db.update_status(&finding_id, new_status, notes)?;
            db.save(db_path)?;

            println!("Updated finding {} to status: {}", finding_id, new_status);
            Ok(ExitCode::SUCCESS)
        }

        TriageAction::Report {
            format,
            group_by: _,
            output,
        } => {
            let db = TriageDatabase::load(db_path)?;

            let report = match ReportFormat::from_str(&format) {
                Some(ReportFormat::Markdown) => generate_markdown_report(&db),
                Some(ReportFormat::Json) => generate_json_report(&db)?,
                Some(ReportFormat::Text) => generate_text_report(&db),
                None => {
                    eprintln!("Unknown format: {}. Use md, json, or text.", format);
                    return Ok(ExitCode::from(1));
                }
            };

            if let Some(output_path) = output {
                std::fs::write(&output_path, &report)?;
                println!("Report written to: {}", output_path.display());
            } else {
                println!("{}", report);
            }

            Ok(ExitCode::SUCCESS)
        }

        TriageAction::Import {
            input,
            repo,
            exclude_defaults,
            exclude_patterns,
            no_snippets,
            dry_run,
        } => {
            let mut db = TriageDatabase::load(db_path)?;

            // Build exclude patterns list
            let mut patterns: Vec<String> = exclude_patterns;
            if exclude_defaults {
                patterns.extend(
                    move_clippy::triage::DEFAULT_EXCLUDE_PATTERNS
                        .iter()
                        .map(|s| s.to_string()),
                );
            }

            // Read JSON lint output
            let contents = std::fs::read_to_string(&input)?;
            let diagnostics: Vec<JsonDiagnostic> = serde_json::from_str(&contents)?;

            let mut imported = 0;
            let mut excluded = 0;

            for diag in diagnostics {
                // Check if path should be excluded
                if !patterns.is_empty()
                    && move_clippy::triage::should_exclude_path(&diag.file, &patterns)
                {
                    excluded += 1;
                    continue;
                }

                let mut finding = Finding::new(
                    diag.lint.clone(),
                    infer_category(&diag.lint),
                    repo.clone(),
                    diag.file.clone(),
                    diag.row as u32,
                    diag.column as u32,
                    diag.message.clone(),
                );

                // Capture snippet if enabled
                if !no_snippets {
                    let file_path = std::path::Path::new(&diag.file);
                    if let Some((snippet, start_line)) =
                        move_clippy::triage::extract_snippet(file_path, diag.row as u32, 2)
                    {
                        finding.snippet = Some(snippet);
                        finding.snippet_start_line = Some(start_line);
                    }
                }

                if !dry_run {
                    db.add_or_update(finding);
                }
                imported += 1;
            }

            if dry_run {
                println!(
                    "DRY RUN - would import {} findings ({} excluded by patterns)",
                    imported, excluded
                );
            } else {
                db.save(db_path)?;
                println!(
                    "Imported {} findings from {} into {}",
                    imported,
                    input.display(),
                    db_path.display()
                );
                if excluded > 0 {
                    println!("Excluded {} findings by path patterns", excluded);
                }
            }

            Ok(ExitCode::SUCCESS)
        }

        TriageAction::Summary => {
            let db = TriageDatabase::load(db_path)?;
            let summary = db.summary();

            println!("TRIAGE SUMMARY");
            println!("==============");
            println!("Database: {}", db_path.display());
            println!();
            println!("Total Findings: {}", summary.total);
            println!();
            println!("By Status:");
            println!(
                "  Needs Review:   {} ({:.1}%)",
                summary.needs_review,
                pct(summary.needs_review, summary.total)
            );
            println!(
                "  Confirmed:      {} ({:.1}%)",
                summary.confirmed,
                pct(summary.confirmed, summary.total)
            );
            println!(
                "  False Positive: {} ({:.1}%)",
                summary.false_positive,
                pct(summary.false_positive, summary.total)
            );
            println!(
                "  Won't Fix:      {} ({:.1}%)",
                summary.wont_fix,
                pct(summary.wont_fix, summary.total)
            );

            Ok(ExitCode::SUCCESS)
        }

        TriageAction::Stats {
            by,
            min_count,
            sort,
        } => {
            let db = TriageDatabase::load(db_path)?;

            // Collect stats based on grouping
            let mut stats: Vec<(String, usize, usize, usize, usize)> = Vec::new(); // (name, total, confirmed, fp, needs_review)

            match by.as_str() {
                "lint" => {
                    for (lint, findings) in db.group_by_lint() {
                        let total = findings.len();
                        let confirmed = findings
                            .iter()
                            .filter(|f| f.status == TriageStatus::Confirmed)
                            .count();
                        let fp = findings
                            .iter()
                            .filter(|f| f.status == TriageStatus::FalsePositive)
                            .count();
                        let nr = findings
                            .iter()
                            .filter(|f| f.status == TriageStatus::NeedsReview)
                            .count();
                        if total >= min_count {
                            stats.push((lint, total, confirmed, fp, nr));
                        }
                    }
                }
                "repo" => {
                    for (repo, findings) in db.group_by_repo() {
                        let total = findings.len();
                        let confirmed = findings
                            .iter()
                            .filter(|f| f.status == TriageStatus::Confirmed)
                            .count();
                        let fp = findings
                            .iter()
                            .filter(|f| f.status == TriageStatus::FalsePositive)
                            .count();
                        let nr = findings
                            .iter()
                            .filter(|f| f.status == TriageStatus::NeedsReview)
                            .count();
                        if total >= min_count {
                            stats.push((repo, total, confirmed, fp, nr));
                        }
                    }
                }
                "category" => {
                    let mut by_cat: std::collections::HashMap<String, Vec<&Finding>> =
                        std::collections::HashMap::new();
                    for f in db.list_all() {
                        by_cat.entry(f.category.clone()).or_default().push(f);
                    }
                    for (cat, findings) in by_cat {
                        let total = findings.len();
                        let confirmed = findings
                            .iter()
                            .filter(|f| f.status == TriageStatus::Confirmed)
                            .count();
                        let fp = findings
                            .iter()
                            .filter(|f| f.status == TriageStatus::FalsePositive)
                            .count();
                        let nr = findings
                            .iter()
                            .filter(|f| f.status == TriageStatus::NeedsReview)
                            .count();
                        if total >= min_count {
                            stats.push((cat, total, confirmed, fp, nr));
                        }
                    }
                }
                _ => {
                    eprintln!("Unknown grouping: {}. Use lint, repo, or category.", by);
                    return Ok(ExitCode::from(1));
                }
            }

            // Sort
            match sort.as_str() {
                "total" => stats.sort_by(|a, b| b.1.cmp(&a.1)),
                "confirmed" => stats.sort_by(|a, b| b.2.cmp(&a.2)),
                "fp" => stats.sort_by(|a, b| b.3.cmp(&a.3)),
                "fp_rate" => {
                    stats.sort_by(|a, b| {
                        let reviewed_a = a.2 + a.3;
                        let reviewed_b = b.2 + b.3;
                        let rate_a = if reviewed_a > 0 {
                            a.3 as f64 / reviewed_a as f64
                        } else {
                            0.0
                        };
                        let rate_b = if reviewed_b > 0 {
                            b.3 as f64 / reviewed_b as f64
                        } else {
                            0.0
                        };
                        rate_b
                            .partial_cmp(&rate_a)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                }
                _ => stats.sort_by(|a, b| b.1.cmp(&a.1)),
            }

            // Print header
            let header = match by.as_str() {
                "lint" => "Lint",
                "repo" => "Repository",
                "category" => "Category",
                _ => "Name",
            };

            println!(
                "{:<35} {:>6} {:>6} {:>6} {:>6} {:>7}",
                header, "Total", "Conf", "FP", "NR", "FP%"
            );
            println!("{}", "-".repeat(75));

            for (name, total, confirmed, fp, nr) in &stats {
                let reviewed = *confirmed + *fp;
                let fp_rate = if reviewed > 0 {
                    (*fp as f64 / reviewed as f64) * 100.0
                } else {
                    0.0
                };
                let truncated = if name.len() > 35 {
                    format!("{}...", &name[..32])
                } else {
                    name.clone()
                };
                println!(
                    "{:<35} {:>6} {:>6} {:>6} {:>6} {:>6.1}%",
                    truncated, total, confirmed, fp, nr, fp_rate
                );
            }

            println!("{}", "-".repeat(75));
            let totals: (usize, usize, usize, usize) = stats.iter().fold((0, 0, 0, 0), |acc, x| {
                (acc.0 + x.1, acc.1 + x.2, acc.2 + x.3, acc.3 + x.4)
            });
            println!(
                "{:<35} {:>6} {:>6} {:>6} {:>6}",
                "TOTAL", totals.0, totals.1, totals.2, totals.3
            );

            Ok(ExitCode::SUCCESS)
        }

        TriageAction::BulkUpdate {
            lint,
            path_contains,
            repo,
            current_status,
            status,
            notes,
            dry_run,
        } => {
            let mut db = TriageDatabase::load(db_path)?;

            // Parse the new status
            let new_status = TriageStatus::from_str(&status)?;

            // Parse current status filter if provided
            let current_status_filter = if let Some(ref cs) = current_status {
                Some(TriageStatus::from_str(cs)?)
            } else {
                None
            };

            // Collect matching finding IDs
            let mut matching_ids: Vec<String> = Vec::new();

            for finding in db.list_all() {
                // Apply filters
                if let Some(ref l) = lint
                    && &finding.lint != l
                {
                    continue;
                }

                if let Some(ref p) = path_contains
                    && !finding.file.contains(p)
                {
                    continue;
                }

                if let Some(ref r) = repo
                    && &finding.repo != r
                {
                    continue;
                }

                if let Some(cs) = current_status_filter
                    && finding.status != cs
                {
                    continue;
                }

                matching_ids.push(finding.id.clone());
            }

            if matching_ids.is_empty() {
                println!("No findings match the specified criteria.");
                return Ok(ExitCode::SUCCESS);
            }

            if dry_run {
                println!(
                    "DRY RUN - would update {} findings to status '{}':",
                    matching_ids.len(),
                    new_status
                );
                for id in matching_ids.iter().take(10) {
                    if let Some(f) = db.get(id) {
                        println!("  {} {} {}:{}", f.lint, f.repo, f.file, f.line);
                    }
                }
                if matching_ids.len() > 10 {
                    println!("  ... and {} more", matching_ids.len() - 10);
                }
            } else {
                // Apply updates
                for id in &matching_ids {
                    db.update_status(id, new_status, notes.clone())?;
                }

                db.save(db_path)?;
                println!(
                    "Updated {} findings to status '{}'",
                    matching_ids.len(),
                    new_status
                );
            }

            Ok(ExitCode::SUCCESS)
        }
    }
}

/// Calculate percentage, avoiding division by zero.
fn pct(count: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        (count as f64 / total as f64) * 100.0
    }
}

/// Infer lint category from lint name (fallback when not available).
fn infer_category(lint: &str) -> String {
    // Security lints
    if lint.contains("hot_potato")
        || lint.contains("capability")
        || lint.contains("oracle")
        || lint.contains("overflow")
        || lint.contains("token_abilities")
        || lint.contains("ownership_transfer")
        || lint.contains("witness")
        || lint.contains("random")
        || lint.contains("coin_split")
        || lint.contains("division")
        || lint.contains("access_control")
        || lint.contains("return_value")
    {
        return "security".to_string();
    }

    // Modernization lints
    if lint.contains("modern") || lint.contains("prefer_") || lint.contains("empty_vector") {
        return "modernization".to_string();
    }

    // Test lints
    if lint.contains("test") {
        return "test_quality".to_string();
    }

    // Naming lints
    if lint.contains("naming")
        || lint.contains("suffix")
        || lint.contains("prefix")
        || lint.contains("constant_")
    {
        return "naming".to_string();
    }

    // Default to style
    "style".to_string()
}

fn lint_file_text(
    engine: &LintEngine,
    path: &Path,
    format: OutputFormat,
    deny_warnings: bool,
    show_tier: bool,
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
                let tier_prefix = if show_tier {
                    format!("[{}] ", diag.lint.group.as_str())
                } else {
                    String::new()
                };
                println!(
                    "{}:{}:{}: {}: {}{}: {}",
                    file,
                    diag.span.start.row,
                    diag.span.start.column,
                    diag.level.as_str(),
                    tier_prefix,
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

                let lint_name = if show_tier {
                    format!("[{}]{}", diag.lint.group.as_str(), diag.lint.name)
                } else {
                    diag.lint.name.to_string()
                };

                println!(
                    "::{} file={},line={},col={},title={}::{}",
                    kind,
                    github_escape(&file),
                    diag.span.start.row,
                    diag.span.start.column,
                    lint_name,
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
    show_tier: bool,
) -> anyhow::Result<(usize, bool)> {
    let mut source = String::new();
    std::io::stdin().read_to_string(&mut source)?;
    let diagnostics = engine.lint_source(&source)?;

    let mut has_error = false;

    match format {
        OutputFormat::Pretty => {
            for diag in &diagnostics {
                let file = diag.file.clone().unwrap_or_else(|| "stdin".to_string());
                let tier_prefix = if show_tier {
                    format!("[{}] ", diag.lint.group.as_str())
                } else {
                    String::new()
                };
                println!(
                    "{}:{}:{}: {}: {}{}: {}",
                    file,
                    diag.span.start.row,
                    diag.span.start.column,
                    diag.level.as_str(),
                    tier_prefix,
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

                let lint_name = if show_tier {
                    format!("[{}]{}", diag.lint.group.as_str(), diag.lint.name)
                } else {
                    diag.lint.name.to_string()
                };

                println!(
                    "::{} file={},line={},col={},title={}::{}",
                    kind,
                    github_escape(&file),
                    diag.span.start.row,
                    diag.span.start.column,
                    lint_name,
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
