use crate::diagnostics::Span;
use crate::lint::{LintCategory, LintContext, LintDescriptor, LintRule};
use tree_sitter::Node;

use super::util::{
    is_exact_test_attr, is_expected_failure_attr, is_only_whitespace_between,
    position_from_byte_offset, slice, walk,
};

pub struct MergeTestAttributesLint;

static MERGE_TEST_ATTRIBUTES: LintDescriptor = LintDescriptor {
    name: "merge_test_attributes",
    category: LintCategory::TestQuality,
    description: "Merge stacked #[test] and #[expected_failure] into a single attribute list",
};

impl LintRule for MergeTestAttributesLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &MERGE_TEST_ATTRIBUTES
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        let mut attrs: Vec<(usize, usize)> = Vec::new();
        walk(root, &mut |node| {
            let t = slice(source, node).trim();
            if t.starts_with("#[") && t.ends_with(']') {
                attrs.push((node.start_byte(), node.end_byte()));
            }
        });

        attrs.sort_by_key(|(start, _end)| *start);
        for pair in attrs.windows(2) {
            let (a_start, a_end) = pair[0];
            let (b_start, b_end) = pair[1];

            let a_text = source.get(a_start..a_end).unwrap_or("");
            let b_text = source.get(b_start..b_end).unwrap_or("");

            if !is_exact_test_attr(a_text) {
                continue;
            }
            if !is_expected_failure_attr(b_text) {
                continue;
            }
            if !is_only_whitespace_between(source, a_end, b_start) {
                continue;
            }

            let span = Span {
                start: position_from_byte_offset(source, a_start),
                end: position_from_byte_offset(source, b_end),
            };

            ctx.report(
                self.descriptor(),
                span,
                "Merge `#[test]` and `#[expected_failure]` into `#[test, expected_failure]`",
            );
        }
    }
}
