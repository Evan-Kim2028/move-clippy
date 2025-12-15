//! Centralized annotation handling for move-clippy.
//!
//! This module provides parsing and handling of Move attributes that control
//! lint behavior:
//!
//! - `#[allow(lint::name)]` - Suppress a specific lint
//! - `#[deny(lint::name)]` - Promote a lint to error level
//! - `#[expect(lint::name)]` - Expect a warning, error if not triggered
//! - `#[validates(param)]` - Mark function as validating a capability parameter
//!
//! Module-level annotations use `#!` syntax:
//! - `#![allow(lint::style)]` - Suppress all style lints in module

use std::collections::HashSet;

/// Annotation types recognized by move-clippy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveClippyAnnotation {
    /// `#[allow(lint::name)]` - Suppress warnings for this lint
    Allow(String),
    /// `#[deny(lint::name)]` - Promote this lint to error level
    Deny(String),
    /// `#[expect(lint::name)]` - Expect this lint to fire, error if it doesn't
    Expect(String),
    /// `#[validates(param_name)]` - Mark function as validating a capability parameter
    Validates(String),
}

impl MoveClippyAnnotation {
    /// Get the lint name for Allow/Deny/Expect annotations.
    pub fn lint_name(&self) -> Option<&str> {
        match self {
            MoveClippyAnnotation::Allow(name)
            | MoveClippyAnnotation::Deny(name)
            | MoveClippyAnnotation::Expect(name) => Some(name),
            MoveClippyAnnotation::Validates(_) => None,
        }
    }

    /// Get the parameter name for Validates annotations.
    pub fn validates_param(&self) -> Option<&str> {
        match self {
            MoveClippyAnnotation::Validates(param) => Some(param),
            _ => None,
        }
    }
}

/// Parse annotations from attribute text before an item.
///
/// Scans backwards from `item_start_byte` to find all annotations
/// in the attribute block preceding the item.
pub fn parse_annotations(source: &str, item_start_byte: usize) -> Vec<MoveClippyAnnotation> {
    let Some(before_item) = source.get(..item_start_byte) else {
        return Vec::new();
    };

    // Keep scan local to avoid picking up unrelated earlier attributes
    let mut start = before_item.len().saturating_sub(4096);
    while start > 0 && !before_item.is_char_boundary(start) {
        start -= 1;
    }
    let window = &before_item[start..];

    let mut annotations = Vec::new();

    for line in window.lines().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Skip doc comments
        if trimmed.starts_with("///")
            || trimmed.starts_with("/**")
            || trimmed.starts_with('*')
            || trimmed.starts_with("*/")
        {
            continue;
        }

        // Parse attribute lines
        if trimmed.starts_with("#[") || trimmed.starts_with("#![") {
            if let Some(ann) = parse_annotation_line(trimmed) {
                annotations.push(ann);
            }
            continue;
        }

        // Any other line means we've left the attribute block
        break;
    }

    annotations
}

/// Parse a single annotation line.
fn parse_annotation_line(line: &str) -> Option<MoveClippyAnnotation> {
    let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();

    // #[allow(lint::name)] or #![allow(lint::name)]
    if let Some(rest) = compact.strip_prefix("#[allow(lint::")
        && let Some(name) = rest.strip_suffix(")]")
    {
        return Some(MoveClippyAnnotation::Allow(name.to_string()));
    }
    if let Some(rest) = compact.strip_prefix("#![allow(lint::")
        && let Some(name) = rest.strip_suffix(")]")
    {
        return Some(MoveClippyAnnotation::Allow(name.to_string()));
    }

    // #[deny(lint::name)]
    if let Some(rest) = compact.strip_prefix("#[deny(lint::")
        && let Some(name) = rest.strip_suffix(")]")
    {
        return Some(MoveClippyAnnotation::Deny(name.to_string()));
    }
    if let Some(rest) = compact.strip_prefix("#![deny(lint::")
        && let Some(name) = rest.strip_suffix(")]")
    {
        return Some(MoveClippyAnnotation::Deny(name.to_string()));
    }

    // #[expect(lint::name)]
    if let Some(rest) = compact.strip_prefix("#[expect(lint::")
        && let Some(name) = rest.strip_suffix(")]")
    {
        return Some(MoveClippyAnnotation::Expect(name.to_string()));
    }
    if let Some(rest) = compact.strip_prefix("#![expect(lint::")
        && let Some(name) = rest.strip_suffix(")]")
    {
        return Some(MoveClippyAnnotation::Expect(name.to_string()));
    }

    // #[validates(param)]
    if let Some(rest) = compact.strip_prefix("#[validates(")
        && let Some(param) = rest.strip_suffix(")]")
    {
        return Some(MoveClippyAnnotation::Validates(param.to_string()));
    }

    None
}

/// Get the validates annotation for a function, if present.
pub fn get_validates_annotation(source: &str, fn_start_byte: usize) -> Option<String> {
    let annotations = parse_annotations(source, fn_start_byte);
    annotations.into_iter().find_map(|a| match a {
        MoveClippyAnnotation::Validates(param) => Some(param),
        _ => None,
    })
}

/// Suppression scope for tracking active suppressions during lint traversal.
#[derive(Debug, Default, Clone)]
pub struct SuppressionScope {
    /// Lints that are allowed (suppressed) in this scope
    allowed: HashSet<String>,
    /// Lints that are denied (promoted to error) in this scope
    denied: HashSet<String>,
    /// Lints that are expected (must fire or error) in this scope
    expected: HashSet<String>,
    /// Parameters marked as validated by this scope
    validated_params: HashSet<String>,
}

impl SuppressionScope {
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a scope from a list of annotations.
    pub fn from_annotations(annotations: Vec<MoveClippyAnnotation>) -> Self {
        let mut scope = Self::new();
        for ann in annotations {
            match ann {
                MoveClippyAnnotation::Allow(name) => {
                    scope.allowed.insert(name);
                }
                MoveClippyAnnotation::Deny(name) => {
                    scope.denied.insert(name);
                }
                MoveClippyAnnotation::Expect(name) => {
                    scope.expected.insert(name);
                }
                MoveClippyAnnotation::Validates(param) => {
                    scope.validated_params.insert(param);
                }
            }
        }
        scope
    }

    /// Check if a lint is suppressed in this scope.
    pub fn is_suppressed(&self, lint_name: &str) -> bool {
        self.allowed.contains(lint_name)
    }

    /// Check if a lint is denied (promoted to error) in this scope.
    pub fn is_denied(&self, lint_name: &str) -> bool {
        self.denied.contains(lint_name)
    }

    /// Check if a lint is expected in this scope.
    pub fn is_expected(&self, lint_name: &str) -> bool {
        self.expected.contains(lint_name)
    }

    /// Check if a parameter is marked as validated.
    pub fn is_validated(&self, param_name: &str) -> bool {
        self.validated_params.contains(param_name)
    }

    /// Merge another scope into this one (for nested scopes).
    pub fn merge(&mut self, other: &SuppressionScope) {
        self.allowed.extend(other.allowed.iter().cloned());
        self.denied.extend(other.denied.iter().cloned());
        self.expected.extend(other.expected.iter().cloned());
        self.validated_params
            .extend(other.validated_params.iter().cloned());
    }

    /// Get all expected lints that haven't fired.
    pub fn unfired_expectations(&self) -> impl Iterator<Item = &String> {
        self.expected.iter()
    }

    /// Mark an expected lint as having fired.
    pub fn mark_expected_fired(&mut self, lint_name: &str) {
        self.expected.remove(lint_name);
    }
}

/// Stack of suppression scopes for hierarchical scope tracking.
#[derive(Debug, Default)]
pub struct SuppressionStack {
    scopes: Vec<SuppressionScope>,
}

impl SuppressionStack {
    pub fn new() -> Self {
        Self { scopes: Vec::new() }
    }

    /// Push a new scope onto the stack.
    pub fn push(&mut self, scope: SuppressionScope) {
        self.scopes.push(scope);
    }

    /// Pop a scope from the stack.
    pub fn pop(&mut self) -> Option<SuppressionScope> {
        self.scopes.pop()
    }

    /// Check if a lint is suppressed in any active scope.
    pub fn is_suppressed(&self, lint_name: &str) -> bool {
        self.scopes.iter().any(|s| s.is_suppressed(lint_name))
    }

    /// Check if a lint is denied in any active scope.
    pub fn is_denied(&self, lint_name: &str) -> bool {
        self.scopes.iter().any(|s| s.is_denied(lint_name))
    }

    /// Check if a parameter is validated in any active scope.
    pub fn is_validated(&self, param_name: &str) -> bool {
        self.scopes.iter().any(|s| s.is_validated(param_name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_allow_annotation() {
        let source = r#"
    #[allow(lint::unused_capability_param)]
    public fun foo() {}
"#;
        let fn_start = source.find("public fun").unwrap();
        let annotations = parse_annotations(source, fn_start);

        assert_eq!(annotations.len(), 1);
        assert!(matches!(
            &annotations[0],
            MoveClippyAnnotation::Allow(name) if name == "unused_capability_param"
        ));
    }

    #[test]
    fn test_parse_deny_annotation() {
        let source = r#"
    #[deny(lint::unsafe_arithmetic)]
    public fun bar() {}
"#;
        let fn_start = source.find("public fun").unwrap();
        let annotations = parse_annotations(source, fn_start);

        assert_eq!(annotations.len(), 1);
        assert!(matches!(
            &annotations[0],
            MoveClippyAnnotation::Deny(name) if name == "unsafe_arithmetic"
        ));
    }

    #[test]
    fn test_parse_validates_annotation() {
        let source = r#"
    #[validates(admin_cap)]
    fun check_admin(cap: &AdminCap) {}
"#;
        let fn_start = source.find("fun check_admin").unwrap();
        let annotations = parse_annotations(source, fn_start);

        assert_eq!(annotations.len(), 1);
        assert!(matches!(
            &annotations[0],
            MoveClippyAnnotation::Validates(param) if param == "admin_cap"
        ));
    }

    #[test]
    fn test_parse_multiple_annotations() {
        let source = r#"
    #[allow(lint::style)]
    #[deny(lint::security)]
    #[validates(cap)]
    public fun baz() {}
"#;
        let fn_start = source.find("public fun").unwrap();
        let annotations = parse_annotations(source, fn_start);

        assert_eq!(annotations.len(), 3);
    }

    #[test]
    fn test_suppression_scope() {
        let annotations = vec![
            MoveClippyAnnotation::Allow("lint_a".to_string()),
            MoveClippyAnnotation::Deny("lint_b".to_string()),
            MoveClippyAnnotation::Validates("cap".to_string()),
        ];

        let scope = SuppressionScope::from_annotations(annotations);

        assert!(scope.is_suppressed("lint_a"));
        assert!(!scope.is_suppressed("lint_b"));
        assert!(scope.is_denied("lint_b"));
        assert!(scope.is_validated("cap"));
    }

    #[test]
    fn test_suppression_stack() {
        let mut stack = SuppressionStack::new();

        let scope1 = SuppressionScope::from_annotations(vec![MoveClippyAnnotation::Allow(
            "lint_a".to_string(),
        )]);
        stack.push(scope1);

        assert!(stack.is_suppressed("lint_a"));
        assert!(!stack.is_suppressed("lint_b"));

        let scope2 = SuppressionScope::from_annotations(vec![MoveClippyAnnotation::Allow(
            "lint_b".to_string(),
        )]);
        stack.push(scope2);

        assert!(stack.is_suppressed("lint_a"));
        assert!(stack.is_suppressed("lint_b"));

        stack.pop();

        assert!(stack.is_suppressed("lint_a"));
        assert!(!stack.is_suppressed("lint_b"));
    }

    #[test]
    fn test_module_level_annotation() {
        let source = r#"
#![allow(lint::style)]
module example::test {
    public fun foo() {}
}
"#;
        // Module-level annotation should be parsed
        let module_start = source.find("module").unwrap();
        let annotations = parse_annotations(source, module_start);

        assert_eq!(annotations.len(), 1);
        assert!(matches!(
            &annotations[0],
            MoveClippyAnnotation::Allow(name) if name == "style"
        ));
    }

    #[test]
    fn test_get_validates_annotation() {
        let source = r#"
    #[validates(admin_cap)]
    fun validate_admin(cap: &AdminCap): bool {}
"#;
        let fn_start = source.find("fun validate").unwrap();
        let validates = get_validates_annotation(source, fn_start);

        assert_eq!(validates, Some("admin_cap".to_string()));
    }
}
