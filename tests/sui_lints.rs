#![cfg(feature = "full")]

use insta::assert_snapshot;
use move_clippy::diagnostics::Diagnostic;
use move_clippy::error::{ClippyResult, MoveClippyError};
use move_clippy::instrument_block;
use move_clippy::lint::LintSettings;
use move_clippy::semantic;
use move_compiler::editions::Flavor;
use move_package::{BuildConfig, compilation::build_plan::BuildPlan};
use regex::Regex;
use std::fs;
use std::path::{Path, PathBuf};

const FIXTURE_ROOT: &str = "../mysten-move-snapshot/crates/move-compiler/tests/sui_mode/linter";

macro_rules! sui_fixture_tests {
    ($($name:ident,)+) => {
        $(
            #[test]
            fn $name() {
                let output = run_fixture(stringify!($name)).expect("fixture should compile");
                assert_snapshot!(stringify!($name), output);
            }
        )+
    };
}

sui_fixture_tests! {
    coin_field,
    collection_equality,
    custom_state_change,
    edge_case_lint_missing_key,
    false_negative_lint_missing_key,
    false_positive_share_owned,
    false_unnecessary_public_entry,
    freeze_wrapped,
    freezing_capability_false_negatives,
    freezing_capability_false_positives,
    freezing_capability_suppression,
    freezing_capability_true_negatives,
    freezing_capability_true_positives,
    lint_all_syntax,
    lint_all_syntax_missing,
    lint_does_not_suppress_compiler_warnings,
    no_trigger_lint_missing_key,
    public_random_invalid,
    public_random_valid,
    self_transfer,
    suppress_lint_missing_key,
    suppress_public_mut_tx_context,
    suppress_unnecessary_public_entry,
    trigger_lint_missing_key,
    true_negative_public_mut_tx_context,
    true_negative_share_owned,
    true_positive_public_mut_tx_context,
    true_positive_share_owned,
    true_unnecessary_public_entry,
}

fn run_fixture(name: &str) -> ClippyResult<String> {
    let manifest_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source = manifest_root
        .join(FIXTURE_ROOT)
        .join(format!("{name}.move"));
    instrument_block!("tests::run_fixture", {
        let package_dir = tempfile::tempdir()?;
        write_manifest(package_dir.path(), "legacy")?;

        let sources = package_dir.path().join("sources");
        fs::create_dir_all(&sources)?;
        fs::copy(&source, sources.join("main.move"))?;

        modernize_fixture(package_dir.path())?;
        write_manifest(package_dir.path(), "2024")?;

        let mut diagnostics =
            semantic::lint_package(package_dir.path(), &LintSettings::default(), false, false)?;

        // These fixtures come from the upstream Move compiler's Sui linter suite. Keep the
        // snapshots focused on delegated Sui lints (not move-clippy's additional semantic lints).
        diagnostics.retain(|d| d.lint.description.starts_with("[Sui Linter]"));

        Ok(format_diagnostics(package_dir.path(), diagnostics))
    })
}

fn write_manifest(dir: &Path, edition: &str) -> ClippyResult<()> {
    let manifest = format!(
        r#"[package]
 name = "sui_lint_fixture"
 edition = "{edition}"

[addresses]
a = "0x0"
b = "0x1"
c = "0x2"
d = "0x3"
e = "0x4"
f = "0x5"
g = "0x6"
h = "0x7"
s = "0x8"
std = "0x1"
sui = "0x2"
sui_system = "0x3"
bridge = "0x4"
deepbook = "0x5"
clock = "0x6"
authenticator_state = "0x7"
test = "0x8"
random = "0x9"
"#,
    );

    fs::write(dir.join("Move.toml"), manifest)?;
    Ok(())
}

fn modernize_fixture(root: &Path) -> ClippyResult<()> {
    instrument_block!("tests::modernize_fixture", {
        run_migration(root)?;
        rewrite_lint_attributes(root)?;
        Ok(())
    })
}

#[allow(clippy::field_reassign_with_default)] // Intentional for clarity
fn run_migration(root: &Path) -> ClippyResult<()> {
    let mut config = BuildConfig::default();
    config.dev_mode = true;
    config.test_mode = true;
    config.default_flavor = Some(Flavor::Sui);

    let mut writer = Vec::new();
    let resolved = config.resolution_graph_for_package(root, None, &mut writer)?;
    let build_plan = BuildPlan::create(&resolved)?;

    if let Some(mut migration) = build_plan.migrate(&mut writer)? {
        let mut sink = std::io::sink();
        migration.apply_changes(&mut sink)?;
    }

    Ok(())
}

fn rewrite_lint_attributes(root: &Path) -> ClippyResult<()> {
    let path = root.join("sources/main.move");
    let contents = fs::read_to_string(&path)?;
    let converted = convert_lint_allow_attributes(&contents)?;
    let rewritten = expand_multi_lint_invocations(&converted)?;
    fs::write(path, rewritten)?;
    Ok(())
}

fn convert_lint_allow_attributes(source: &str) -> ClippyResult<String> {
    let regex = Regex::new(r"(?m)(?P<prefix>^\s*)#\[\s*lint_allow\s*\((?P<args>[^)]+)\)\s*]")
        .map_err(|e| MoveClippyError::fixture(format!("invalid lint attribute pattern: {e}")))?;
    let result = regex.replace_all(source, |caps: &regex::Captures| {
        let converted = format_lint_invocations(&caps["args"]);
        format!("{}#[allow({converted})]", &caps["prefix"])
    });
    Ok(result.into_owned())
}

fn expand_multi_lint_invocations(source: &str) -> ClippyResult<String> {
    let regex = Regex::new(r"lint\s*\((?P<args>[^)]+,[^)]*)\)")
        .map_err(|e| MoveClippyError::fixture(format!("invalid lint invocation pattern: {e}")))?;
    let result = regex.replace_all(source, |caps: &regex::Captures| {
        format_lint_invocations(&caps["args"])
    });
    Ok(result.into_owned())
}

fn format_lint_invocations(args: &str) -> String {
    args.split(',')
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .map(|part| format!("lint({part})"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn format_diagnostics(root: &Path, mut diagnostics: Vec<Diagnostic>) -> String {
    let canonical_root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    diagnostics.sort_by(|a, b| {
        let a_file = a.file.as_deref().unwrap_or("");
        let b_file = b.file.as_deref().unwrap_or("");
        (a_file, a.span.start.row, a.span.start.column, a.lint.name).cmp(&(
            b_file,
            b.span.start.row,
            b.span.start.column,
            b.lint.name,
        ))
    });

    diagnostics
        .into_iter()
        .map(|diag| {
            let file = diag.file.unwrap_or_else(|| "<unknown>".to_string());
            let display = relativize(&canonical_root, &file);
            let mut line = format!(
                "{display}:{row}:{col}: {}: {}",
                diag.lint.name,
                diag.message,
                row = diag.span.start.row,
                col = diag.span.start.column,
            );
            if let Some(help) = diag.help
                && !help.is_empty()
            {
                line.push_str("\n  help: ");
                line.push_str(&help);
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn relativize(root: &Path, file: &str) -> String {
    let path = PathBuf::from(file);
    let canonical = path.canonicalize().unwrap_or(path);
    match canonical.strip_prefix(root) {
        Ok(suffix) => suffix.display().to_string(),
        Err(_) => canonical.display().to_string(),
    }
}
