//! Auto-fix infrastructure for applying code transformations.
//!
//! This module provides a pure functional approach to applying fixes to source code.
//! All functions work on strings and byte offsets - no file I/O.
//!
//! ## Safety Guarantees
//!
//! - Edits are validated to be non-overlapping before application
//! - Edits are applied in reverse order to preserve byte offsets
//! - All operations are pure (no side effects)

use thiserror::Error;

/// Error type for fix application operations.
#[derive(Debug, Error)]
pub enum FixError {
    #[error("Overlapping edits detected at byte {0}")]
    OverlappingEdits(usize),

    #[error("Edit range [{start}..{end}) exceeds source length {source_len}")]
    InvalidRange {
        start: usize,
        end: usize,
        source_len: usize,
    },

    #[error("Edit start {start} is after edit end {end}")]
    InvalidEditOrder { start: usize, end: usize },
}

/// Represents a text edit to apply to source code.
///
/// Edits are defined by byte offsets (not character offsets) for efficiency
/// with tree-sitter's byte-based API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextEdit {
    /// Starting byte offset (inclusive).
    pub start_byte: usize,
    /// Ending byte offset (exclusive).
    pub end_byte: usize,
    /// Text to insert in place of the range [start_byte..end_byte).
    pub replacement: String,
}

impl TextEdit {
    /// Create a new text edit.
    pub fn new(start_byte: usize, end_byte: usize, replacement: String) -> Self {
        Self {
            start_byte,
            end_byte,
            replacement,
        }
    }

    /// Create a deletion edit (removes text, inserts nothing).
    pub fn delete(start_byte: usize, end_byte: usize) -> Self {
        Self::new(start_byte, end_byte, String::new())
    }

    /// Create an insertion edit (inserts text at a position).
    pub fn insert(byte_offset: usize, text: String) -> Self {
        Self::new(byte_offset, byte_offset, text)
    }

    /// Create a replacement edit (replaces a range with new text).
    pub fn replace(start_byte: usize, end_byte: usize, replacement: String) -> Self {
        Self::new(start_byte, end_byte, replacement)
    }

    /// Returns the byte range affected by this edit.
    pub fn range(&self) -> std::ops::Range<usize> {
        self.start_byte..self.end_byte
    }

    /// Returns true if this edit overlaps with another.
    pub fn overlaps_with(&self, other: &TextEdit) -> bool {
        // Two ranges [a, b) and [c, d) overlap if:
        // a < d && c < b
        self.start_byte < other.end_byte && other.start_byte < self.end_byte
    }

    /// Validates that this edit has a valid range.
    pub fn validate(&self, source_len: usize) -> Result<(), FixError> {
        if self.start_byte > self.end_byte {
            return Err(FixError::InvalidEditOrder {
                start: self.start_byte,
                end: self.end_byte,
            });
        }

        if self.end_byte > source_len {
            return Err(FixError::InvalidRange {
                start: self.start_byte,
                end: self.end_byte,
                source_len,
            });
        }

        Ok(())
    }
}

/// Validate that a list of edits are non-overlapping and within bounds.
pub fn validate_edits(edits: &[TextEdit], source_len: usize) -> Result<(), FixError> {
    // Validate each edit individually
    for edit in edits {
        edit.validate(source_len)?;
    }

    // Check for overlaps between edits
    for i in 0..edits.len() {
        for j in (i + 1)..edits.len() {
            if edits[i].overlaps_with(&edits[j]) {
                return Err(FixError::OverlappingEdits(edits[i].start_byte));
            }
        }
    }

    Ok(())
}

/// Apply a list of non-overlapping edits to source code.
///
/// Edits are sorted by start_byte in descending order before application.
/// This ensures that earlier byte offsets remain valid as we apply edits.
///
/// # Errors
///
/// Returns an error if:
/// - Edits overlap with each other
/// - Any edit has an invalid range
/// - Any edit exceeds source length
///
/// # Example
///
/// ```rust
/// use move_clippy::fix::{TextEdit, apply_fixes};
///
/// let source = "while (true) { break }";
/// let edits = vec![
///     TextEdit::replace(0, 12, "loop".to_string()),
/// ];
///
/// let result = apply_fixes(source, &edits).unwrap();
/// assert_eq!(result, "loop { break }");
/// ```
pub fn apply_fixes(source: &str, edits: &[TextEdit]) -> Result<String, FixError> {
    if edits.is_empty() {
        return Ok(source.to_string());
    }

    // Validate all edits first
    validate_edits(edits, source.len())?;

    // Sort edits by start_byte in descending order (apply from end to start)
    let mut sorted_edits = edits.to_vec();
    sorted_edits.sort_by(|a, b| b.start_byte.cmp(&a.start_byte));

    let mut result = source.to_string();

    // Apply edits from end to start to preserve byte offsets
    for edit in sorted_edits {
        // Remove the range [start_byte..end_byte)
        result.drain(edit.start_byte..edit.end_byte);
        // Insert the replacement at start_byte
        result.insert_str(edit.start_byte, &edit.replacement);
    }

    Ok(result)
}

/// Apply a single edit to source code (convenience wrapper).
pub fn apply_fix(source: &str, edit: &TextEdit) -> Result<String, FixError> {
    apply_fixes(source, std::slice::from_ref(edit))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_edit_creation() {
        let edit = TextEdit::new(0, 5, "hello".to_string());
        assert_eq!(edit.start_byte, 0);
        assert_eq!(edit.end_byte, 5);
        assert_eq!(edit.replacement, "hello");
    }

    #[test]
    fn test_text_edit_delete() {
        let edit = TextEdit::delete(10, 20);
        assert_eq!(edit.start_byte, 10);
        assert_eq!(edit.end_byte, 20);
        assert_eq!(edit.replacement, "");
    }

    #[test]
    fn test_text_edit_insert() {
        let edit = TextEdit::insert(5, "inserted".to_string());
        assert_eq!(edit.start_byte, 5);
        assert_eq!(edit.end_byte, 5);
        assert_eq!(edit.replacement, "inserted");
    }

    #[test]
    fn test_overlaps_with() {
        let edit1 = TextEdit::new(0, 10, "a".to_string());
        let edit2 = TextEdit::new(5, 15, "b".to_string());
        let edit3 = TextEdit::new(10, 20, "c".to_string());

        assert!(edit1.overlaps_with(&edit2));
        assert!(edit2.overlaps_with(&edit1));
        assert!(!edit1.overlaps_with(&edit3));
        assert!(!edit3.overlaps_with(&edit1));
    }

    #[test]
    fn test_validate_edit_valid() {
        let edit = TextEdit::new(0, 5, "hello".to_string());
        assert!(edit.validate(10).is_ok());
    }

    #[test]
    fn test_validate_edit_invalid_order() {
        let edit = TextEdit::new(10, 5, "hello".to_string());
        assert!(matches!(
            edit.validate(20),
            Err(FixError::InvalidEditOrder { .. })
        ));
    }

    #[test]
    fn test_validate_edit_exceeds_length() {
        let edit = TextEdit::new(0, 15, "hello".to_string());
        assert!(matches!(
            edit.validate(10),
            Err(FixError::InvalidRange { .. })
        ));
    }

    #[test]
    fn test_validate_edits_overlapping() {
        let edits = vec![
            TextEdit::new(0, 10, "a".to_string()),
            TextEdit::new(5, 15, "b".to_string()),
        ];
        assert!(matches!(
            validate_edits(&edits, 20),
            Err(FixError::OverlappingEdits(_))
        ));
    }

    #[test]
    fn test_validate_edits_non_overlapping() {
        let edits = vec![
            TextEdit::new(0, 5, "a".to_string()),
            TextEdit::new(10, 15, "b".to_string()),
        ];
        assert!(validate_edits(&edits, 20).is_ok());
    }

    #[test]
    fn test_apply_single_replacement() {
        let source = "while (true) { break }";
        let edit = TextEdit::replace(0, 12, "loop".to_string());
        let result = apply_fix(source, &edit).unwrap();
        assert_eq!(result, "loop { break }");
    }

    #[test]
    fn test_apply_single_deletion() {
        let source = "hello world";
        let edit = TextEdit::delete(5, 11);
        let result = apply_fix(source, &edit).unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_apply_single_insertion() {
        let source = "hello world";
        let edit = TextEdit::insert(5, " beautiful".to_string());
        let result = apply_fix(source, &edit).unwrap();
        assert_eq!(result, "hello beautiful world");
    }

    #[test]
    fn test_apply_multiple_edits_preserves_offsets() {
        let source = "one two three";
        let edits = vec![
            TextEdit::replace(0, 3, "1".to_string()),  // "one" → "1"
            TextEdit::replace(4, 7, "2".to_string()),  // "two" → "2"
            TextEdit::replace(8, 13, "3".to_string()), // "three" → "3"
        ];
        let result = apply_fixes(source, &edits).unwrap();
        assert_eq!(result, "1 2 3");
    }

    #[test]
    fn test_apply_edits_reversed_order() {
        // Edits should work regardless of input order
        let source = "abc def ghi";
        let edits = vec![
            TextEdit::replace(8, 11, "3".to_string()), // Last edit
            TextEdit::replace(0, 3, "1".to_string()),  // First edit
            TextEdit::replace(4, 7, "2".to_string()),  // Middle edit
        ];
        let result = apply_fixes(source, &edits).unwrap();
        assert_eq!(result, "1 2 3");
    }

    #[test]
    fn test_idempotency_no_edits() {
        let source = "unchanged";
        let result1 = apply_fixes(source, &[]).unwrap();
        let result2 = apply_fixes(&result1, &[]).unwrap();
        assert_eq!(result1, source);
        assert_eq!(result2, source);
    }

    #[test]
    fn test_roundtrip_property() {
        // If we replace A with B, then B with A, we get back to original
        let source = "while (true) { }";

        let edit1 = TextEdit::replace(0, 12, "loop".to_string());
        let result1 = apply_fix(source, &edit1).unwrap();
        assert_eq!(result1, "loop { }");

        let edit2 = TextEdit::replace(0, 4, "while (true)".to_string());
        let result2 = apply_fix(&result1, &edit2).unwrap();
        assert_eq!(result2, "while (true) { }");
    }
}
