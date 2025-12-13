use crate::lint::{FixDescriptor, LintCategory, LintContext, LintDescriptor, LintRule, RuleGroup};
use tree_sitter::Node;

use super::util::{slice, walk};

// ============================================================================
// AdminCapPositionLint - P1 (Low FP)
// ============================================================================

pub struct AdminCapPositionLint;

static ADMIN_CAP_POSITION: LintDescriptor = LintDescriptor {
    name: "admin_cap_position",
    category: LintCategory::Style,
    description: "Capability parameters should be first (or second after TxContext)",
    group: RuleGroup::Stable,
    fix: FixDescriptor::none(),
};

impl LintRule for AdminCapPositionLint {
    fn descriptor(&self) -> &'static LintDescriptor {
        &ADMIN_CAP_POSITION
    }

    fn check(&self, root: Node, source: &str, ctx: &mut LintContext<'_>) {
        walk(root, &mut |node| {
            if node.kind() != "function_definition" {
                return;
            }

            let Some(params) = node.child_by_field_name("parameters") else {
                return;
            };

            // Collect all parameters with their types
            let mut param_infos: Vec<ParamInfo> = Vec::new();
            let mut cursor = params.walk();

            for param in params.children(&mut cursor) {
                if param.kind() != "function_parameter" && param.kind() != "mut_function_parameter"
                {
                    continue;
                }

                let Some(ty) = param.child_by_field_name("type") else {
                    continue;
                };

                let type_text = slice(source, ty).trim();
                param_infos.push(ParamInfo {
                    node: param,
                    type_text: type_text.to_string(),
                    is_capability: is_capability_type(type_text),
                    is_tx_context: is_tx_context_type(type_text),
                });
            }

            // Check for capability in wrong position
            check_cap_position(&param_infos, ctx, &ADMIN_CAP_POSITION);
        });
    }
}

struct ParamInfo<'a> {
    node: Node<'a>,
    type_text: String,
    is_capability: bool,
    is_tx_context: bool,
}

fn check_cap_position(
    params: &[ParamInfo],
    ctx: &mut LintContext<'_>,
    lint: &'static LintDescriptor,
) {
    if params.is_empty() {
        return;
    }

    // Find the first capability parameter
    let cap_index = params.iter().position(|p| p.is_capability);
    let Some(cap_idx) = cap_index else {
        return; // No capability parameter
    };

    // Check if TxContext is first
    let tx_context_first = params.first().map(|p| p.is_tx_context).unwrap_or(false);

    // Expected position: 0 if no TxContext first, 1 if TxContext is first
    let expected_pos = if tx_context_first { 1 } else { 0 };

    // If capability is not in expected position, report
    if cap_idx != expected_pos && cap_idx > expected_pos {
        let cap_param = &params[cap_idx];

        let position_hint = if tx_context_first {
            "second (after TxContext)"
        } else {
            "first"
        };

        ctx.report_node(
            lint,
            cap_param.node,
            format!(
                "Capability parameter `{}` should be {} in the parameter list",
                cap_param.type_text, position_hint
            ),
        );
    }
}

/// Check if a type looks like a capability (ends with Cap or Capability)
fn is_capability_type(type_text: &str) -> bool {
    let cleaned = type_text
        .trim()
        .trim_start_matches('&')
        .trim_start_matches("mut ")
        .trim();

    // Check for common capability suffixes
    cleaned.ends_with("Cap")
        || cleaned.ends_with("Capability")
        || cleaned.ends_with("_cap")
        || cleaned.ends_with("AdminCap")
        || cleaned.ends_with("OwnerCap")
        || cleaned.ends_with("TreasuryCap")
}

/// Check if a type is TxContext
fn is_tx_context_type(type_text: &str) -> bool {
    let cleaned = type_text
        .trim()
        .trim_start_matches('&')
        .trim_start_matches("mut ")
        .trim();

    cleaned == "TxContext" || cleaned.ends_with("::TxContext")
}
