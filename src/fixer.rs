//! Auto-fix application module.
//!
//! This module handles applying machine-applicable fix suggestions to source files.

use crate::diagnostics::{Applicability, Diagnostic};
use std::path::Path;

/// Result of applying fixes to a source file.
#[derive(Debug)]
pub struct FixResult {
    /// The modified source code.
    pub fixed_source: String,
    /// Number of fixes applied.
    pub fixes_applied: usize,
    /// Fix suggestions that were skipped (not machine-applicable or unsafe).
    pub fixes_skipped: usize,
}

/// Error when applying fixes.
#[derive(Debug, thiserror::Error)]
pub enum FixError {
    #[error("Cannot apply fixes to stdin - please specify a file path")]
    StdinNotSupported,

    #[error("Overlapping fixes detected - cannot safely apply")]
    OverlappingFixes,

    #[error("Invalid byte range: {0}..{1}")]
    InvalidRange(usize, usize),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// A text edit to apply to source code.
#[derive(Debug, Clone)]
pub struct SourceEdit {
    /// Start byte offset (0-based, inclusive).
    pub start_byte: usize,
    /// End byte offset (0-based, exclusive).
    pub end_byte: usize,
    /// Replacement text.
    pub replacement: String,
}

impl SourceEdit {
    pub fn new(start: usize, end: usize, replacement: impl Into<String>) -> Self {
        Self {
            start_byte: start,
            end_byte: end,
            replacement: replacement.into(),
        }
    }
}

/// Apply fixes from diagnostics to source code.
///
/// # Arguments
/// * `source` - The original source code
/// * `diagnostics` - Diagnostics with fix suggestions
/// * `allow_unsafe` - Whether to apply unsafe fixes
///
/// # Returns
/// A `FixResult` containing the modified source and statistics
pub fn apply_fixes(
    source: &str,
    diagnostics: &[Diagnostic],
    allow_unsafe: bool,
) -> Result<FixResult, FixError> {
    // Collect applicable edits
    let mut edits: Vec<SourceEdit> = Vec::new();
    let mut skipped = 0;

    for diag in diagnostics {
        let Some(suggestion) = &diag.suggestion else {
            continue;
        };

        // Check applicability
        match suggestion.applicability {
            Applicability::MachineApplicable => {
                // Always apply
            }
            Applicability::MaybeIncorrect | Applicability::HasPlaceholders => {
                if !allow_unsafe {
                    skipped += 1;
                    continue;
                }
            }
            Applicability::Unspecified => {
                skipped += 1;
                continue;
            }
        }

        // Convert row/column span to byte offsets
        let Some((start_byte, end_byte)) = span_to_bytes(source, &diag.span) else {
            skipped += 1;
            continue;
        };

        edits.push(SourceEdit::new(
            start_byte,
            end_byte,
            &suggestion.replacement,
        ));
    }

    if edits.is_empty() {
        return Ok(FixResult {
            fixed_source: source.to_string(),
            fixes_applied: 0,
            fixes_skipped: skipped,
        });
    }

    // Sort edits by start position (descending) to apply from end to start
    edits.sort_by(|a, b| b.start_byte.cmp(&a.start_byte));

    // Check for overlapping edits
    for window in edits.windows(2) {
        // Since sorted descending, window[0].start > window[1].start
        // Overlap if window[0].start < window[1].end
        if window[0].start_byte < window[1].end_byte {
            return Err(FixError::OverlappingFixes);
        }
    }

    // Apply edits from end to start
    let mut result = source.to_string();
    let applied = edits.len();

    for edit in edits {
        if edit.start_byte > result.len() || edit.end_byte > result.len() {
            return Err(FixError::InvalidRange(edit.start_byte, edit.end_byte));
        }

        result.replace_range(edit.start_byte..edit.end_byte, &edit.replacement);
    }

    Ok(FixResult {
        fixed_source: result,
        fixes_applied: applied,
        fixes_skipped: skipped,
    })
}

/// Convert a row/column span to byte offsets.
///
/// Spans use 1-based row and column numbers.
fn span_to_bytes(source: &str, span: &crate::diagnostics::Span) -> Option<(usize, usize)> {
    let mut byte_offset = 0;
    let mut current_row = 1;
    let mut current_col = 1;

    let mut start_byte = None;
    let mut end_byte = None;

    for (i, c) in source.char_indices() {
        // Check if we're at the start position
        if current_row == span.start.row && current_col == span.start.column {
            start_byte = Some(i);
        }

        // Check if we're at the end position
        if current_row == span.end.row && current_col == span.end.column {
            end_byte = Some(i);
            break;
        }

        // Update position
        if c == '\n' {
            current_row += 1;
            current_col = 1;
        } else {
            current_col += 1;
        }

        byte_offset = i + c.len_utf8();
    }

    // Handle end position at EOF
    if end_byte.is_none() && current_row == span.end.row && current_col == span.end.column {
        end_byte = Some(byte_offset);
    }

    // Handle end position after last char on line
    if end_byte.is_none() && start_byte.is_some() {
        end_byte = Some(source.len());
    }

    match (start_byte, end_byte) {
        (Some(s), Some(e)) => Some((s, e)),
        _ => None,
    }
}

/// Generate a unified diff between original and fixed source.
///
/// Includes context lines (3 lines before and after each change) for better readability.
pub fn format_diff(original: &str, fixed: &str, path: &Path) -> String {
    format_diff_with_context(original, fixed, path, 3)
}

/// Generate a unified diff with configurable context lines.
pub fn format_diff_with_context(
    original: &str,
    fixed: &str,
    path: &Path,
    context: usize,
) -> String {
    use std::fmt::Write;

    let path_str = path.display().to_string();
    let mut output = String::new();

    writeln!(output, "--- a/{}", path_str).unwrap();
    writeln!(output, "+++ b/{}", path_str).unwrap();

    let orig_lines: Vec<&str> = original.lines().collect();
    let fixed_lines: Vec<&str> = fixed.lines().collect();

    // Find all changed line indices
    let mut changes: Vec<(usize, Option<&str>, Option<&str>)> = Vec::new();
    let max_len = orig_lines.len().max(fixed_lines.len());

    for i in 0..max_len {
        let orig = orig_lines.get(i).copied();
        let fix = fixed_lines.get(i).copied();

        if orig != fix {
            changes.push((i, orig, fix));
        }
    }

    if changes.is_empty() {
        return String::new();
    }

    // Group changes into hunks with context
    let mut hunks: Vec<(usize, usize, Vec<(usize, Option<&str>, Option<&str>)>)> = Vec::new();
    let mut current_hunk_start = 0usize;
    let mut current_hunk_end = 0usize;
    let mut current_changes: Vec<(usize, Option<&str>, Option<&str>)> = Vec::new();

    for (i, orig, fix) in changes {
        let change_start = i.saturating_sub(context);
        let change_end = (i + context + 1).min(max_len);

        if current_changes.is_empty() {
            // Start new hunk
            current_hunk_start = change_start;
            current_hunk_end = change_end;
            current_changes.push((i, orig, fix));
        } else if change_start <= current_hunk_end {
            // Extend current hunk
            current_hunk_end = change_end;
            current_changes.push((i, orig, fix));
        } else {
            // Save current hunk, start new one
            hunks.push((current_hunk_start, current_hunk_end, current_changes));
            current_hunk_start = change_start;
            current_hunk_end = change_end;
            current_changes = vec![(i, orig, fix)];
        }
    }

    // Don't forget the last hunk
    if !current_changes.is_empty() {
        hunks.push((current_hunk_start, current_hunk_end, current_changes));
    }

    // Output each hunk
    for (hunk_start, hunk_end, hunk_changes) in hunks {
        let change_indices: std::collections::HashSet<usize> =
            hunk_changes.iter().map(|(i, _, _)| *i).collect();

        // Calculate hunk sizes
        let orig_size = hunk_end.min(orig_lines.len()).saturating_sub(hunk_start);
        let fixed_size = hunk_end.min(fixed_lines.len()).saturating_sub(hunk_start);

        // Hunk header
        writeln!(
            output,
            "@@ -{},{} +{},{} @@",
            hunk_start + 1,
            orig_size,
            hunk_start + 1,
            fixed_size
        )
        .unwrap();

        // Output lines with context
        for line_idx in hunk_start..hunk_end {
            if change_indices.contains(&line_idx) {
                // This is a changed line
                if let Some(orig) = orig_lines.get(line_idx) {
                    writeln!(output, "-{}", orig).unwrap();
                }
                if let Some(fix) = fixed_lines.get(line_idx) {
                    writeln!(output, "+{}", fix).unwrap();
                }
            } else {
                // Context line (unchanged)
                if let Some(line) = orig_lines.get(line_idx) {
                    writeln!(output, " {}", line).unwrap();
                }
            }
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    // Static descriptor for tests
    static TEST_LINT: crate::lint::LintDescriptor = crate::lint::LintDescriptor {
        name: "test",
        category: crate::lint::LintCategory::Style,
        description: "test",
        group: crate::lint::RuleGroup::Stable,
        fix: crate::lint::FixDescriptor::none(),
    };

    #[test]
    fn test_span_to_bytes_simple() {
        let source = "line1\nline2\nline3";
        let span = crate::diagnostics::Span {
            start: crate::diagnostics::Position { row: 2, column: 1 },
            end: crate::diagnostics::Position { row: 2, column: 6 },
        };

        let result = span_to_bytes(source, &span);
        assert_eq!(result, Some((6, 11))); // "line2" starts at byte 6
    }

    #[test]
    fn test_apply_single_fix() {
        let source = "let x = vector::empty();";

        // Simulate a diagnostic with fix
        // "vector::empty()" is at columns 9-24 (1-based, exclusive end)
        let diag = Diagnostic {
            lint: &TEST_LINT,
            level: crate::level::LintLevel::Warn,
            file: None,
            span: crate::diagnostics::Span {
                start: crate::diagnostics::Position { row: 1, column: 9 },
                end: crate::diagnostics::Position { row: 1, column: 24 },
            },
            message: "test".into(),
            help: None,
            suggestion: Some(crate::diagnostics::Suggestion {
                message: "Replace".into(),
                replacement: "vector[]".into(),
                applicability: Applicability::MachineApplicable,
            }),
        };

        let result = apply_fixes(source, &[diag], false).unwrap();
        assert_eq!(result.fixed_source, "let x = vector[];");
        assert_eq!(result.fixes_applied, 1);
    }

    #[test]
    fn test_format_diff() {
        let original = "let x = vector::empty();\nlet y = 1;";
        let fixed = "let x = vector[];\nlet y = 1;";
        let path = Path::new("test.move");

        let diff = format_diff(original, fixed, path);
        assert!(diff.contains("--- a/test.move"));
        assert!(diff.contains("+++ b/test.move"));
        assert!(diff.contains("-let x = vector::empty();"));
        assert!(diff.contains("+let x = vector[];"));
    }
}
