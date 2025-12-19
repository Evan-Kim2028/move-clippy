use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_rs_files(&path, out)?;
            continue;
        }
        if file_type.is_file() && path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
    Ok(())
}

#[test]
fn no_ctx_report_diagnostic_calls_in_src_rules() {
    let rules_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("rules");

    let mut rs_files = Vec::new();
    collect_rs_files(&rules_dir, &mut rs_files).expect("should list src/rules/**/*.rs");
    rs_files.sort();
    assert!(
        !rs_files.is_empty(),
        "expected rust files under {rules_dir:?}"
    );

    let needle = "ctx.report_diagnostic(";
    let mut hits = Vec::new();

    for path in rs_files {
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));

        for (idx, line) in content.lines().enumerate() {
            if line.contains(needle) {
                hits.push(format!("{}:{}: {}", path.display(), idx + 1, line.trim()));
            }
        }
    }

    assert!(
        hits.is_empty(),
        "suppression bypass regression: found `{needle}` call sites under src/rules:\n{}",
        hits.join("\n")
    );
}

fn collect_md_files_shallow(dir: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_file() && path.extension().is_some_and(|ext| ext == "md") {
            out.push(path);
        }
    }
    Ok(())
}

fn line_looks_like_inventory_count(line: &str) -> bool {
    let trimmed = line.trim_start();
    let patterns = [
        "**Total Lints:**",
        "Total Lints:",
        "Total lints:",
        "- Total:",
        "- Total lints:",
        "**Stable:**",
        "Stable:",
        "- Stable:",
        "**Preview:**",
        "Preview:",
        "- Preview:",
        "**Experimental:**",
        "Experimental:",
        "- Experimental:",
        "**Deprecated:**",
        "Deprecated:",
        "- Deprecated:",
    ];
    for pat in patterns {
        if let Some(rest) = trimmed.strip_prefix(pat) {
            let rest = rest.trim_start_matches([' ', '|']).trim();
            if rest.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                return true;
            }
        }
    }
    false
}

#[test]
fn generated_docs_have_generation_headers() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs");

    let lint_reference = root.join("LINT_REFERENCE.md");
    let content = fs::read_to_string(&lint_reference)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", lint_reference.display()));
    assert!(
        content.contains("**Status:** Generated"),
        "expected {} to declare generated status",
        lint_reference.display()
    );
    assert!(
        content.contains("cargo run --features full --bin gen_lint_reference"),
        "expected {} to include regen command",
        lint_reference.display()
    );

    let catalog_summary = root.join("LINT_CATALOG_SUMMARY.md");
    let content = fs::read_to_string(&catalog_summary)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", catalog_summary.display()));
    assert!(
        content.contains("**Status:** Generated"),
        "expected {} to declare generated status",
        catalog_summary.display()
    );
    assert!(
        content.contains("cargo run --features full --bin gen_lint_catalog_summary"),
        "expected {} to include regen command",
        catalog_summary.display()
    );
}

#[test]
fn no_manual_lint_inventory_counts_in_docs_root() {
    let docs_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("docs");
    let allowlisted = [
        docs_dir.join("LINT_REFERENCE.md"),
        docs_dir.join("LINT_CATALOG_SUMMARY.md"),
    ];

    let mut md_files = Vec::new();
    collect_md_files_shallow(&docs_dir, &mut md_files).expect("should list docs/*.md");
    md_files.sort();

    let mut hits = Vec::new();
    for path in md_files {
        if allowlisted.iter().any(|p| p == &path) {
            continue;
        }
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
        for (idx, line) in content.lines().enumerate() {
            if line_looks_like_inventory_count(line) {
                hits.push(format!("{}:{}: {}", path.display(), idx + 1, line.trim()));
            }
        }
    }

    assert!(
        hits.is_empty(),
        "manual lint inventory counts drift risk: keep counts in generated docs only:\n{}",
        hits.join("\n")
    );
}

#[cfg(feature = "full")]
#[test]
fn absint_diag_codes_are_mapped() {
    for (code, _descriptor) in move_clippy::absint_lints::ABSINT_CUSTOM_DIAG_CODE_MAP {
        assert!(
            move_clippy::absint_lints::descriptor_for_diag_code(*code).is_some(),
            "expected absint diag code {code} to map to a descriptor"
        );
    }
}
