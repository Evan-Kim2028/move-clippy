use crate::lint::LintDescriptor;
use tree_sitter::Range;

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub lint: &'static LintDescriptor,
    pub span: Span,
    pub message: String,
    pub help: Option<String>,
    pub suggestion: Option<Suggestion>,
}

#[derive(Debug, Clone)]
pub struct Suggestion {
    pub message: String,
    pub replacement: String,
    pub applicability: Applicability,
}

#[derive(Debug, Clone, Copy)]
pub enum Applicability {
    MachineApplicable,
    MaybeIncorrect,
    HasPlaceholders,
    Unspecified,
}

#[derive(Debug, Clone, Copy)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}

#[derive(Debug, Clone, Copy)]
pub struct Position {
    pub row: usize,
    pub column: usize,
}

impl Span {
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
