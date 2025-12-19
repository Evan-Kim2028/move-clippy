use crate::level::LintLevel;
use crate::lint::LintDescriptor;
use tree_sitter::Range;

/// A single lint finding produced by Move Clippy.
#[derive(Debug, Clone)]
#[must_use]
pub struct Diagnostic {
    pub lint: &'static LintDescriptor,
    pub level: LintLevel,
    pub file: Option<String>,
    pub span: Span,
    pub message: String,
    pub help: Option<String>,
    pub suggestion: Option<Suggestion>,
}

/// Optional machine- or human-applicable fix for a diagnostic.
#[derive(Debug, Clone)]
pub struct Suggestion {
    pub message: String,
    pub replacement: String,
    pub applicability: Applicability,
}

/// Applicability of an automated suggestion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applicability {
    MachineApplicable,
    MaybeIncorrect,
    HasPlaceholders,
    Unspecified,
}

/// Span in a Move source file (1-based row/column positions).
#[derive(Debug, Clone, Copy)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}

/// Single position in a Move source file (1-based row/column).
#[derive(Debug, Clone, Copy)]
pub struct Position {
    pub row: usize,
    pub column: usize,
}

impl Span {
    /// Construct a `Span` from a tree-sitter range, converting to 1-based positions.
    #[must_use]
    pub fn from_range(range: Range) -> Self {
        Self {
            start: Position {
                row: range.start_point.row + 1,
                column: range.start_point.column + 1,
            },
            end: Position {
                row: range.end_point.row + 1,
                column: range.end_point.column + 1,
            },
        }
    }
}
