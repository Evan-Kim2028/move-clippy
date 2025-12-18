//! Ecosystem tests for Move Clippy.
//!
//! These tests run move-clippy against real-world Move repositories to ensure
//! that changes don't introduce regressions (new false positives or missed detections).
//!
//! # How it works
//!
//! 1. Each ecosystem repo has a baseline JSON file in `tests/baselines/`
//! 2. Tests run move-clippy against the repo and compare to baseline
//! 3. New violations not in baseline cause test failure
//! 4. To update baselines, run: `UPDATE_BASELINES=1 cargo test ecosystem`
//!
//! # Adding a new ecosystem repo
//!
//! 1. Clone the repo to `../` relative to move-clippy
//! 2. Create a baseline: `UPDATE_BASELINES=1 cargo test ecosystem_<name>`
//! 3. Commit the baseline JSON file

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// A single diagnostic from move-clippy, normalized for comparison.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct BaselineDiagnostic {
    /// Relative path from repo root
    pub file: String,
    /// Lint rule name
    pub lint: String,
    /// Line number (1-indexed)
    pub line: usize,
    /// Diagnostic message
    pub message: String,
}

/// Baseline for a single ecosystem repository.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EcosystemBaseline {
    /// Repository name
    pub repo: String,
    /// Git commit hash when baseline was generated
    pub commit: Option<String>,
    /// Expected diagnostics
    pub diagnostics: BTreeSet<BaselineDiagnostic>,
}

/// Configuration for an ecosystem test.
pub struct EcosystemTest {
    /// Name of the repository
    pub name: &'static str,
    /// Path to the repository relative to move-clippy crate root
    pub repo_path: &'static str,
    /// Subdirectory within repo to lint (e.g., "sources" or "packages")
    pub lint_path: Option<&'static str>,
}

impl EcosystemTest {
    /// Get the absolute path to the repository.
    fn repo_abs_path(&self) -> PathBuf {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest_dir.join(self.repo_path)
    }

    /// Get the absolute path to lint within the repository.
    fn lint_abs_path(&self) -> PathBuf {
        let repo = self.repo_abs_path();
        match self.lint_path {
            Some(subpath) => repo.join(subpath),
            None => repo,
        }
    }

    /// Get the baseline file path.
    fn baseline_path(&self) -> PathBuf {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest_dir
            .join("tests/baselines")
            .join(format!("{}.json", self.name))
    }

    /// Run move-clippy and collect diagnostics.
    fn run_linter(&self) -> anyhow::Result<Vec<BaselineDiagnostic>> {
        use move_clippy::LintEngine;
        use move_clippy::lint::{LintRegistry, LintSettings};

        let lint_path = self.lint_abs_path();
        if !lint_path.exists() {
            anyhow::bail!(
                "Repository path does not exist: {}. Clone it first.",
                lint_path.display()
            );
        }

        let registry = LintRegistry::default_rules();
        let engine = LintEngine::new_with_settings(registry, LintSettings::default());

        let mut diagnostics = Vec::new();
        let repo_root = self.repo_abs_path();

        for entry in walkdir::WalkDir::new(&lint_path)
            .into_iter()
            .filter_entry(|e| !is_ignored_dir(e.path()))
        {
            let entry = entry?;
            if !entry.file_type().is_file() {
                continue;
            }
            if entry.path().extension().and_then(|s| s.to_str()) != Some("move") {
                continue;
            }

            let source = std::fs::read_to_string(entry.path())?;
            let diags = engine.lint_source(&source)?;

            let relative_path = entry
                .path()
                .strip_prefix(&repo_root)
                .unwrap_or(entry.path())
                .to_string_lossy()
                .to_string();

            for d in diags {
                diagnostics.push(BaselineDiagnostic {
                    file: relative_path.clone(),
                    lint: d.lint.name.to_string(),
                    line: d.span.start.row,
                    message: d.message.clone(),
                });
            }
        }

        diagnostics.sort();
        Ok(diagnostics)
    }

    /// Load or create baseline.
    fn load_baseline(&self) -> anyhow::Result<EcosystemBaseline> {
        let path = self.baseline_path();
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let baseline: EcosystemBaseline = serde_json::from_str(&content)?;
            Ok(baseline)
        } else {
            Ok(EcosystemBaseline {
                repo: self.name.to_string(),
                commit: None,
                diagnostics: BTreeSet::new(),
            })
        }
    }

    /// Save baseline to file.
    fn save_baseline(&self, baseline: &EcosystemBaseline) -> anyhow::Result<()> {
        let path = self.baseline_path();
        let content = serde_json::to_string_pretty(baseline)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Run the ecosystem test.
    pub fn run(&self) -> anyhow::Result<()> {
        let current_diagnostics: BTreeSet<_> = self.run_linter()?.into_iter().collect();
        let baseline = self.load_baseline()?;

        // Check if we should update baselines
        if std::env::var("UPDATE_BASELINES").is_ok() {
            let new_baseline = EcosystemBaseline {
                repo: self.name.to_string(),
                commit: get_git_commit(&self.repo_abs_path()),
                diagnostics: current_diagnostics,
            };
            self.save_baseline(&new_baseline)?;
            println!(
                "Updated baseline for {} with {} diagnostics",
                self.name,
                new_baseline.diagnostics.len()
            );
            return Ok(());
        }

        // Find new diagnostics not in baseline
        let new_diagnostics: Vec<_> = current_diagnostics
            .difference(&baseline.diagnostics)
            .collect();

        // Find removed diagnostics (in baseline but not current)
        let removed_diagnostics: Vec<_> = baseline
            .diagnostics
            .difference(&current_diagnostics)
            .collect();

        if !new_diagnostics.is_empty() {
            println!("\n🚨 NEW DIAGNOSTICS (not in baseline):\n");
            for d in &new_diagnostics {
                println!("  {}:{}: {} - {}", d.file, d.line, d.lint, d.message);
            }
        }

        if !removed_diagnostics.is_empty() {
            println!("\n✅ REMOVED DIAGNOSTICS (were in baseline, now fixed):\n");
            for d in &removed_diagnostics {
                println!("  {}:{}: {} - {}", d.file, d.line, d.lint, d.message);
            }
        }

        if !new_diagnostics.is_empty() {
            anyhow::bail!(
                "Ecosystem test {} failed: {} new diagnostic(s) not in baseline.\n\
                 If these are expected, run: UPDATE_BASELINES=1 cargo test ecosystem_{}",
                self.name,
                new_diagnostics.len(),
                self.name
            );
        }

        println!(
            "✅ Ecosystem test {} passed ({} diagnostics, {} removed)",
            self.name,
            current_diagnostics.len(),
            removed_diagnostics.len()
        );
        Ok(())
    }
}

fn is_ignored_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|s| s.to_str())
        .map(|name| matches!(name, ".git" | "target" | "build" | "node_modules"))
        .unwrap_or(false)
}

fn get_git_commit(repo_path: &Path) -> Option<String> {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_path)
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                String::from_utf8(out.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        })
}

// ============================================================================
// Ecosystem Test Definitions
// ============================================================================
//
// All repos are cloned to ../ecosystem-test-repos/ using the clone-repos.sh script.
// Run: cd ../ecosystem-test-repos && ./clone-repos.sh

/// DeepBook V3 - Sui's native liquidity layer (MystenLabs)
#[test]
#[ignore = "requires ecosystem-test-repos clone"]
fn ecosystem_deepbookv3() {
    let test = EcosystemTest {
        name: "deepbookv3",
        repo_path: "../ecosystem-test-repos/deepbookv3",
        lint_path: Some("packages"),
    };
    test.run().expect("ecosystem test failed");
}

/// OpenZeppelin Sui - Security-focused Move contracts
#[test]
#[ignore = "requires ecosystem-test-repos clone"]
fn ecosystem_openzeppelin_sui() {
    let test = EcosystemTest {
        name: "openzeppelin-sui",
        repo_path: "../ecosystem-test-repos/openzeppelin-sui",
        lint_path: Some("contracts"),
    };
    test.run().expect("ecosystem test failed");
}

/// Cetus CLMM - Concentrated Liquidity AMM
#[test]
#[ignore = "requires ecosystem-test-repos clone"]
fn ecosystem_cetus_clmm() {
    let test = EcosystemTest {
        name: "cetus-clmm",
        repo_path: "../ecosystem-test-repos/cetus-clmm",
        lint_path: Some("sui"),
    };
    test.run().expect("ecosystem test failed");
}

/// Scallop Lending - Sui lending protocol
#[test]
#[ignore = "requires ecosystem-test-repos clone"]
fn ecosystem_scallop_lend() {
    let test = EcosystemTest {
        name: "scallop-lend",
        repo_path: "../ecosystem-test-repos/scallop-lend",
        lint_path: Some("contracts"),
    };
    test.run().expect("ecosystem test failed");
}

// ============================================================================
// Interest Protocol Repos (https://github.com/interest-protocol)
// ============================================================================

/// Suitears - Production-ready Move modules (math, defi, governance, utils)
/// https://github.com/interest-protocol/suitears
#[test]
#[ignore = "requires ecosystem-test-repos clone"]
fn ecosystem_suitears() {
    let test = EcosystemTest {
        name: "suitears",
        repo_path: "../ecosystem-test-repos/suitears",
        lint_path: Some("contracts"),
    };
    test.run().expect("ecosystem test failed");
}

/// Memez.gg - Meme token infrastructure
/// https://github.com/interest-protocol/memez-gg
#[test]
#[ignore = "requires ecosystem-test-repos clone"]
fn ecosystem_memez_gg() {
    let test = EcosystemTest {
        name: "memez-gg",
        repo_path: "../ecosystem-test-repos/memez-gg",
        lint_path: None,
    };
    test.run().expect("ecosystem test failed");
}

/// Interest MVR - Collection of Sui libs for MVR
/// https://github.com/interest-protocol/interest-mvr
#[test]
#[ignore = "requires ecosystem-test-repos clone"]
fn ecosystem_interest_mvr() {
    let test = EcosystemTest {
        name: "interest-mvr",
        repo_path: "../ecosystem-test-repos/interest-mvr",
        lint_path: None,
    };
    test.run().expect("ecosystem test failed");
}

/// Move Interfaces - Interface patterns for Move
/// https://github.com/interest-protocol/move-interfaces
#[test]
#[ignore = "requires ecosystem-test-repos clone"]
fn ecosystem_move_interfaces() {
    let test = EcosystemTest {
        name: "move-interfaces",
        repo_path: "../ecosystem-test-repos/move-interfaces",
        lint_path: None,
    };
    test.run().expect("ecosystem test failed");
}

/// Some - Web3 Social Layer
/// https://github.com/interest-protocol/some
#[test]
#[ignore = "requires ecosystem-test-repos clone"]
fn ecosystem_some() {
    let test = EcosystemTest {
        name: "some",
        repo_path: "../ecosystem-test-repos/some",
        lint_path: None,
    };
    test.run().expect("ecosystem test failed");
}
