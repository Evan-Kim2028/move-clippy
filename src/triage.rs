//! Finding Triage & Reporting System
//!
//! A read-only system for tracking, categorizing, and reporting lint findings
//! during the testing/prototyping phase. No automatic file modifications -
//! purely analysis and documentation.
//!
//! ## Core Concepts
//!
//! - **Finding**: A single lint violation detected in source code
//! - **TriageStatus**: The review state of a finding (needs_review, confirmed, etc.)
//! - **TriageDatabase**: JSON-backed storage for findings and their triage state
//!
//! ## Usage
//!
//! ```bash
//! # List findings needing review
//! move-clippy triage list --status needs_review
//!
//! # Update a finding's status
//! move-clippy triage update abc123 --status confirmed --notes "Real bug"
//!
//! # Generate a report
//! move-clippy triage report --format md
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use thiserror::Error;

// ============================================================================
// Error Types
// ============================================================================

#[derive(Debug, Error)]
pub enum TriageError {
    #[error("Failed to read triage database: {0}")]
    ReadError(#[from] std::io::Error),

    #[error("Failed to parse triage database: {0}")]
    ParseError(#[from] serde_json::Error),

    #[error("Finding not found: {0}")]
    FindingNotFound(String),

    #[error("Invalid status: {0}")]
    InvalidStatus(String),
}

// ============================================================================
// Triage Status
// ============================================================================

/// The review state of a finding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TriageStatus {
    /// Default - finding hasn't been evaluated yet
    #[default]
    NeedsReview,
    /// Validated as a real issue
    Confirmed,
    /// Not actually an issue (helps tune lint)
    FalsePositive,
    /// Real issue but acceptable (documented exception)
    WontFix,
}

impl TriageStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TriageStatus::NeedsReview => "needs_review",
            TriageStatus::Confirmed => "confirmed",
            TriageStatus::FalsePositive => "false_positive",
            TriageStatus::WontFix => "wont_fix",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, TriageError> {
        match s.to_lowercase().as_str() {
            "needs_review" | "needsreview" => Ok(TriageStatus::NeedsReview),
            "confirmed" => Ok(TriageStatus::Confirmed),
            "false_positive" | "falsepositive" | "fp" => Ok(TriageStatus::FalsePositive),
            "wont_fix" | "wontfix" | "won't_fix" => Ok(TriageStatus::WontFix),
            _ => Err(TriageError::InvalidStatus(s.to_string())),
        }
    }

    /// Returns all possible status values
    pub fn all() -> &'static [TriageStatus] {
        &[
            TriageStatus::NeedsReview,
            TriageStatus::Confirmed,
            TriageStatus::FalsePositive,
            TriageStatus::WontFix,
        ]
    }
}

impl std::fmt::Display for TriageStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ============================================================================
// Severity Classification
// ============================================================================

/// Severity level for findings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Critical security issue
    Critical,
    /// High-impact issue
    High,
    /// Medium-impact issue
    #[default]
    Medium,
    /// Low-impact issue (style, minor)
    Low,
    /// Informational only
    Info,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Critical => "critical",
            Severity::High => "high",
            Severity::Medium => "medium",
            Severity::Low => "low",
            Severity::Info => "info",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "critical" => Some(Severity::Critical),
            "high" => Some(Severity::High),
            "medium" => Some(Severity::Medium),
            "low" => Some(Severity::Low),
            "info" => Some(Severity::Info),
            _ => None,
        }
    }

    /// Derive severity from lint category
    pub fn from_lint_category(category: &str) -> Self {
        match category {
            "security" => Severity::High,
            "suspicious" => Severity::Medium,
            "style" | "modernization" | "naming" => Severity::Low,
            "test_quality" => Severity::Info,
            _ => Severity::Medium,
        }
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

// ============================================================================
// Finding
// ============================================================================

/// A single lint finding with triage metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    /// Stable unique identifier (hash of location + lint)
    pub id: String,

    /// Name of the lint rule that generated this finding
    pub lint: String,

    /// Category of the lint (security, style, etc.)
    pub category: String,

    /// Repository name (e.g., "alphalend", "scallop-lend")
    pub repo: String,

    /// Relative file path within the repo
    pub file: String,

    /// Line number (1-based)
    pub line: u32,

    /// Column number (1-based)
    pub column: u32,

    /// The lint message
    pub message: String,

    /// Source code snippet (optional, for context)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,

    /// Triage status
    pub status: TriageStatus,

    /// Severity classification
    pub severity: Severity,

    /// Human notes about this finding
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    /// When the finding was first detected
    pub detected_at: DateTime<Utc>,

    /// When the finding was last reviewed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewed_at: Option<DateTime<Utc>>,

    /// Who reviewed it (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reviewed_by: Option<String>,
}

impl Finding {
    /// Generate a stable ID for a finding based on its location and lint
    pub fn generate_id(lint: &str, repo: &str, file: &str, line: u32) -> String {
        let mut hasher = Sha256::new();
        hasher.update(lint.as_bytes());
        hasher.update(repo.as_bytes());
        hasher.update(file.as_bytes());
        hasher.update(line.to_string().as_bytes());
        let result = hasher.finalize();
        // Use first 8 bytes (16 hex chars) for brevity
        hex::encode(&result[..8])
    }

    /// Create a new finding with default triage status
    pub fn new(
        lint: String,
        category: String,
        repo: String,
        file: String,
        line: u32,
        column: u32,
        message: String,
    ) -> Self {
        let id = Self::generate_id(&lint, &repo, &file, line);
        let severity = Severity::from_lint_category(&category);

        Self {
            id,
            lint,
            category,
            repo,
            file,
            line,
            column,
            message,
            snippet: None,
            status: TriageStatus::NeedsReview,
            severity,
            notes: None,
            detected_at: Utc::now(),
            reviewed_at: None,
            reviewed_by: None,
        }
    }

    /// Update the triage status with optional notes
    pub fn update_status(&mut self, status: TriageStatus, notes: Option<String>) {
        self.status = status;
        self.reviewed_at = Some(Utc::now());
        if let Some(n) = notes {
            self.notes = Some(n);
        }
    }

    /// Short display string for listing
    pub fn short_display(&self) -> String {
        format!(
            "[{}] {} {}:{}:{} - {}",
            self.status.as_str(),
            self.lint,
            self.repo,
            self.file,
            self.line,
            truncate(&self.message, 60)
        )
    }
}

// ============================================================================
// Triage Summary
// ============================================================================

/// Summary statistics for the triage database
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TriageSummary {
    pub total: usize,
    pub needs_review: usize,
    pub confirmed: usize,
    pub false_positive: usize,
    pub wont_fix: usize,
}

impl TriageSummary {
    pub fn from_findings(findings: &[Finding]) -> Self {
        let mut summary = Self::default();
        summary.total = findings.len();

        for finding in findings {
            match finding.status {
                TriageStatus::NeedsReview => summary.needs_review += 1,
                TriageStatus::Confirmed => summary.confirmed += 1,
                TriageStatus::FalsePositive => summary.false_positive += 1,
                TriageStatus::WontFix => summary.wont_fix += 1,
            }
        }

        summary
    }
}

// ============================================================================
// Triage Database
// ============================================================================

/// The main triage database - JSON-backed storage for findings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriageDatabase {
    /// Schema version for forward compatibility
    pub version: String,

    /// When this database was last updated
    pub updated_at: DateTime<Utc>,

    /// Summary statistics (computed, not stored)
    #[serde(skip)]
    summary_cache: Option<TriageSummary>,

    /// All findings indexed by ID
    pub findings: HashMap<String, Finding>,
}

impl Default for TriageDatabase {
    fn default() -> Self {
        Self::new()
    }
}

impl TriageDatabase {
    /// Create a new empty database
    pub fn new() -> Self {
        Self {
            version: "1.0".to_string(),
            updated_at: Utc::now(),
            summary_cache: None,
            findings: HashMap::new(),
        }
    }

    /// Load database from a JSON file
    pub fn load(path: &Path) -> Result<Self, TriageError> {
        if !path.exists() {
            return Ok(Self::new());
        }

        let contents = fs::read_to_string(path)?;
        let db: Self = serde_json::from_str(&contents)?;
        Ok(db)
    }

    /// Save database to a JSON file
    pub fn save(&mut self, path: &Path) -> Result<(), TriageError> {
        self.updated_at = Utc::now();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    /// Add a new finding or update existing one
    ///
    /// If a finding with the same ID exists:
    /// - Preserves the existing triage status and notes
    /// - Updates the message and detected_at if changed
    pub fn add_or_update(&mut self, mut finding: Finding) {
        if let Some(existing) = self.findings.get(&finding.id) {
            // Preserve triage data
            finding.status = existing.status;
            finding.notes = existing.notes.clone();
            finding.reviewed_at = existing.reviewed_at;
            finding.reviewed_by = existing.reviewed_by.clone();
            // Keep original detection time if we've seen this before
            if existing.detected_at < finding.detected_at {
                finding.detected_at = existing.detected_at;
            }
        }

        self.findings.insert(finding.id.clone(), finding);
        self.summary_cache = None; // Invalidate cache
    }

    /// Update the status of a finding by ID
    pub fn update_status(
        &mut self,
        id: &str,
        status: TriageStatus,
        notes: Option<String>,
    ) -> Result<(), TriageError> {
        let finding = self
            .findings
            .get_mut(id)
            .ok_or_else(|| TriageError::FindingNotFound(id.to_string()))?;

        finding.update_status(status, notes);
        self.summary_cache = None;
        Ok(())
    }

    /// Get a finding by ID
    pub fn get(&self, id: &str) -> Option<&Finding> {
        self.findings.get(id)
    }

    /// Get summary statistics
    pub fn summary(&self) -> TriageSummary {
        let findings: Vec<_> = self.findings.values().cloned().collect();
        TriageSummary::from_findings(&findings)
    }

    /// List all findings
    pub fn list_all(&self) -> Vec<&Finding> {
        self.findings.values().collect()
    }

    /// Filter findings by various criteria
    pub fn filter(&self, filter: &FindingFilter) -> Vec<&Finding> {
        self.findings
            .values()
            .filter(|f| filter.matches(f))
            .collect()
    }

    /// Group findings by a field
    pub fn group_by_lint(&self) -> HashMap<String, Vec<&Finding>> {
        let mut groups: HashMap<String, Vec<&Finding>> = HashMap::new();
        for finding in self.findings.values() {
            groups
                .entry(finding.lint.clone())
                .or_default()
                .push(finding);
        }
        groups
    }

    /// Group findings by repository
    pub fn group_by_repo(&self) -> HashMap<String, Vec<&Finding>> {
        let mut groups: HashMap<String, Vec<&Finding>> = HashMap::new();
        for finding in self.findings.values() {
            groups
                .entry(finding.repo.clone())
                .or_default()
                .push(finding);
        }
        groups
    }

    /// Group findings by status
    pub fn group_by_status(&self) -> HashMap<TriageStatus, Vec<&Finding>> {
        let mut groups: HashMap<TriageStatus, Vec<&Finding>> = HashMap::new();
        for finding in self.findings.values() {
            groups.entry(finding.status).or_default().push(finding);
        }
        groups
    }
}

// ============================================================================
// Finding Filter
// ============================================================================

/// Filter criteria for finding queries
#[derive(Debug, Clone, Default)]
pub struct FindingFilter {
    pub status: Option<TriageStatus>,
    pub lint: Option<String>,
    pub repo: Option<String>,
    pub severity: Option<Severity>,
    pub category: Option<String>,
}

impl FindingFilter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_status(mut self, status: TriageStatus) -> Self {
        self.status = Some(status);
        self
    }

    pub fn with_lint(mut self, lint: impl Into<String>) -> Self {
        self.lint = Some(lint.into());
        self
    }

    pub fn with_repo(mut self, repo: impl Into<String>) -> Self {
        self.repo = Some(repo.into());
        self
    }

    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = Some(severity);
        self
    }

    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.category = Some(category.into());
        self
    }

    pub fn matches(&self, finding: &Finding) -> bool {
        if let Some(status) = self.status {
            if finding.status != status {
                return false;
            }
        }

        if let Some(ref lint) = self.lint {
            if &finding.lint != lint {
                return false;
            }
        }

        if let Some(ref repo) = self.repo {
            if &finding.repo != repo {
                return false;
            }
        }

        if let Some(severity) = self.severity {
            if finding.severity != severity {
                return false;
            }
        }

        if let Some(ref category) = self.category {
            if &finding.category != category {
                return false;
            }
        }

        true
    }
}

// ============================================================================
// Report Generation
// ============================================================================

/// Report format options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportFormat {
    Markdown,
    Json,
    Text,
}

impl ReportFormat {
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "md" | "markdown" => Some(ReportFormat::Markdown),
            "json" => Some(ReportFormat::Json),
            "txt" | "text" => Some(ReportFormat::Text),
            _ => None,
        }
    }
}

/// Generate a Markdown report from the triage database
pub fn generate_markdown_report(db: &TriageDatabase) -> String {
    let mut out = String::new();
    let summary = db.summary();

    // Header
    out.push_str("# Move Clippy Triage Report\n\n");
    out.push_str(&format!(
        "**Generated:** {}  \n",
        Utc::now().format("%Y-%m-%d %H:%M UTC")
    ));
    out.push_str(&format!("**Total Findings:** {}\n\n", summary.total));

    // Summary by Status
    out.push_str("## Summary by Status\n\n");
    out.push_str("| Status | Count | % |\n");
    out.push_str("|--------|-------|---|\n");

    let statuses = [
        ("Confirmed", summary.confirmed),
        ("False Positive", summary.false_positive),
        ("Won't Fix", summary.wont_fix),
        ("Needs Review", summary.needs_review),
    ];

    for (name, count) in statuses {
        let pct = if summary.total > 0 {
            (count as f64 / summary.total as f64) * 100.0
        } else {
            0.0
        };
        out.push_str(&format!("| {} | {} | {:.1}% |\n", name, count, pct));
    }
    out.push('\n');

    // Confirmed Issues (High Priority)
    let confirmed: Vec<_> = db
        .filter(&FindingFilter::new().with_status(TriageStatus::Confirmed))
        .into_iter()
        .collect();

    if !confirmed.is_empty() {
        out.push_str("## Confirmed Issues\n\n");

        // Group by lint
        let mut by_lint: HashMap<&str, Vec<&&Finding>> = HashMap::new();
        for f in &confirmed {
            by_lint.entry(&f.lint).or_default().push(f);
        }

        for (lint, findings) in by_lint {
            out.push_str(&format!("### {} ({} findings)\n\n", lint, findings.len()));
            out.push_str("| Repo | File | Line | Notes |\n");
            out.push_str("|------|------|------|-------|\n");

            for f in findings {
                let notes = f.notes.as_deref().unwrap_or("-");
                out.push_str(&format!(
                    "| {} | {} | {} | {} |\n",
                    f.repo,
                    truncate(&f.file, 40),
                    f.line,
                    truncate(notes, 50)
                ));
            }
            out.push('\n');
        }
    }

    // False Positives (Lint Tuning)
    let fps: Vec<_> = db
        .filter(&FindingFilter::new().with_status(TriageStatus::FalsePositive))
        .into_iter()
        .collect();

    if !fps.is_empty() {
        out.push_str("## False Positives (Lint Tuning Needed)\n\n");

        let mut by_lint: HashMap<&str, Vec<&&Finding>> = HashMap::new();
        for f in &fps {
            by_lint.entry(&f.lint).or_default().push(f);
        }

        for (lint, findings) in by_lint {
            out.push_str(&format!("### {} ({} findings)\n\n", lint, findings.len()));

            for f in findings {
                let notes = f.notes.as_deref().unwrap_or("No notes");
                out.push_str(&format!(
                    "- `{}:{}` - {}\n",
                    f.file, f.line, notes
                ));
            }
            out.push('\n');
        }
    }

    // By Repository Summary
    out.push_str("## By Repository\n\n");
    out.push_str("| Repository | Total | Confirmed | FP | Needs Review |\n");
    out.push_str("|------------|-------|-----------|----|--------------|\n");

    let by_repo = db.group_by_repo();
    let mut repos: Vec<_> = by_repo.keys().collect();
    repos.sort();

    for repo in repos {
        let findings = &by_repo[repo];
        let total = findings.len();
        let confirmed = findings.iter().filter(|f| f.status == TriageStatus::Confirmed).count();
        let fp = findings.iter().filter(|f| f.status == TriageStatus::FalsePositive).count();
        let needs_review = findings.iter().filter(|f| f.status == TriageStatus::NeedsReview).count();

        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            repo, total, confirmed, fp, needs_review
        ));
    }
    out.push('\n');

    // By Lint Summary
    out.push_str("## By Lint\n\n");
    out.push_str("| Lint | Total | Confirmed | FP | Needs Review |\n");
    out.push_str("|------|-------|-----------|----|--------------|\n");

    let by_lint = db.group_by_lint();
    let mut lints: Vec<_> = by_lint.iter().collect();
    lints.sort_by(|a, b| b.1.len().cmp(&a.1.len())); // Sort by count desc

    for (lint, findings) in lints.iter().take(20) {
        let total = findings.len();
        let confirmed = findings.iter().filter(|f| f.status == TriageStatus::Confirmed).count();
        let fp = findings.iter().filter(|f| f.status == TriageStatus::FalsePositive).count();
        let needs_review = findings.iter().filter(|f| f.status == TriageStatus::NeedsReview).count();

        out.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            lint, total, confirmed, fp, needs_review
        ));
    }

    out
}

/// Generate a JSON report
pub fn generate_json_report(db: &TriageDatabase) -> Result<String, serde_json::Error> {
    #[derive(Serialize)]
    struct JsonReport {
        generated_at: String,
        summary: TriageSummary,
        by_status: HashMap<String, usize>,
        by_lint: HashMap<String, usize>,
        by_repo: HashMap<String, usize>,
    }

    let summary = db.summary();

    let mut by_status = HashMap::new();
    for status in TriageStatus::all() {
        let count = db.filter(&FindingFilter::new().with_status(*status)).len();
        by_status.insert(status.as_str().to_string(), count);
    }

    let mut by_lint = HashMap::new();
    for (lint, findings) in db.group_by_lint() {
        by_lint.insert(lint, findings.len());
    }

    let mut by_repo = HashMap::new();
    for (repo, findings) in db.group_by_repo() {
        by_repo.insert(repo, findings.len());
    }

    let report = JsonReport {
        generated_at: Utc::now().to_rfc3339(),
        summary,
        by_status,
        by_lint,
        by_repo,
    };

    serde_json::to_string_pretty(&report)
}

/// Generate a plain text report
pub fn generate_text_report(db: &TriageDatabase) -> String {
    let mut out = String::new();
    let summary = db.summary();

    out.push_str("MOVE CLIPPY TRIAGE REPORT\n");
    out.push_str(&"=".repeat(60));
    out.push('\n');
    out.push_str(&format!("Total Findings: {}\n\n", summary.total));

    out.push_str("STATUS SUMMARY:\n");
    out.push_str(&format!("  Confirmed:      {}\n", summary.confirmed));
    out.push_str(&format!("  False Positive: {}\n", summary.false_positive));
    out.push_str(&format!("  Won't Fix:      {}\n", summary.wont_fix));
    out.push_str(&format!("  Needs Review:   {}\n", summary.needs_review));

    out
}

// ============================================================================
// Utility Functions
// ============================================================================

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_finding_id_generation() {
        let id1 = Finding::generate_id("test_lint", "repo1", "file.move", 10);
        let id2 = Finding::generate_id("test_lint", "repo1", "file.move", 10);
        let id3 = Finding::generate_id("test_lint", "repo1", "file.move", 11);

        // Same inputs = same ID
        assert_eq!(id1, id2);
        // Different line = different ID
        assert_ne!(id1, id3);
        // IDs should be 16 hex chars
        assert_eq!(id1.len(), 16);
    }

    #[test]
    fn test_triage_status_roundtrip() {
        for status in TriageStatus::all() {
            let s = status.as_str();
            let parsed = TriageStatus::from_str(s).unwrap();
            assert_eq!(*status, parsed);
        }
    }

    #[test]
    fn test_finding_filter() {
        let finding = Finding::new(
            "test_lint".to_string(),
            "security".to_string(),
            "repo1".to_string(),
            "file.move".to_string(),
            10,
            1,
            "Test message".to_string(),
        );

        // Matches with no filter
        let filter = FindingFilter::new();
        assert!(filter.matches(&finding));

        // Matches with correct lint
        let filter = FindingFilter::new().with_lint("test_lint");
        assert!(filter.matches(&finding));

        // Doesn't match with wrong lint
        let filter = FindingFilter::new().with_lint("other_lint");
        assert!(!filter.matches(&finding));

        // Matches with correct repo
        let filter = FindingFilter::new().with_repo("repo1");
        assert!(filter.matches(&finding));
    }

    #[test]
    fn test_database_add_or_update() {
        let mut db = TriageDatabase::new();

        // Add initial finding
        let mut finding = Finding::new(
            "test_lint".to_string(),
            "style".to_string(),
            "repo1".to_string(),
            "file.move".to_string(),
            10,
            1,
            "Original message".to_string(),
        );
        let id = finding.id.clone();
        db.add_or_update(finding);

        assert_eq!(db.findings.len(), 1);
        assert_eq!(db.get(&id).unwrap().status, TriageStatus::NeedsReview);

        // Update the status
        db.update_status(&id, TriageStatus::Confirmed, Some("Real bug".to_string()))
            .unwrap();

        // Re-add the finding (simulating a new lint run)
        let finding2 = Finding::new(
            "test_lint".to_string(),
            "style".to_string(),
            "repo1".to_string(),
            "file.move".to_string(),
            10,
            1,
            "Updated message".to_string(),
        );
        db.add_or_update(finding2);

        // Status should be preserved
        let updated = db.get(&id).unwrap();
        assert_eq!(updated.status, TriageStatus::Confirmed);
        assert_eq!(updated.notes.as_deref(), Some("Real bug"));
    }

    #[test]
    fn test_summary_calculation() {
        let mut db = TriageDatabase::new();

        for i in 0..10 {
            let mut finding = Finding::new(
                "test_lint".to_string(),
                "style".to_string(),
                "repo1".to_string(),
                format!("file{}.move", i),
                i,
                1,
                format!("Message {}", i),
            );

            // Set different statuses
            finding.status = match i % 4 {
                0 => TriageStatus::NeedsReview,
                1 => TriageStatus::Confirmed,
                2 => TriageStatus::FalsePositive,
                _ => TriageStatus::WontFix,
            };

            db.add_or_update(finding);
        }

        let summary = db.summary();
        assert_eq!(summary.total, 10);
        assert_eq!(summary.needs_review, 3);
        assert_eq!(summary.confirmed, 3);
        assert_eq!(summary.false_positive, 2);
        assert_eq!(summary.wont_fix, 2);
    }
}
