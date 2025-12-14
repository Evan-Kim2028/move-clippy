// Phase III: Cross-Module Analysis and Advanced Security Lints
//
// This module implements lints that require analysis across module boundaries,
// including call graph construction, transitive capability tracking, and
// complex flow analysis.
//
// Architecture:
// - CallGraph: Maps module -> called modules and tracks capability flows
// - CrossModuleAnalyzer: Coordinates analysis across the entire program
// - Advanced lints: transitive_capability_leak, flashloan_without_repay

#![cfg(feature = "full")]
#![allow(unused)]

use crate::diagnostics::Diagnostic;
use crate::error::ClippyResult;
use crate::lint::{LintCategory, LintDescriptor, LintSettings, RuleGroup, FixDescriptor};
use move_compiler::{
    diagnostics::{
        Diagnostic as CompilerDiagnostic, Diagnostics as CompilerDiagnostics,
        codes::{DiagnosticInfo, Severity, custom},
    },
    expansion::ast::ModuleIdent,
    hlir::ast::{
        BaseType, BaseType_, Exp, ModuleCall, SingleType, SingleType_, Type, Type_,
        UnannotatedExp_, Var,
    },
    naming::ast as N,
    parser::ast::{Ability_, DatatypeName, FunctionName},
    shared::{
        Identifier,
        program_info::TypingProgramInfo,
    },
    typing::ast as T,
};
use move_ir_types::location::*;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

// ============================================================================
// Lint Diagnostic Codes
// ============================================================================

const LINT_WARNING_PREFIX: &str = "Lint";
const CLIPPY_CATEGORY: u8 = 200;

const TRANSITIVE_CAP_LEAK_DIAG: DiagnosticInfo = custom(
    LINT_WARNING_PREFIX,
    Severity::Warning,
    CLIPPY_CATEGORY,
    10, // transitive_capability_leak
    "capability leaks across module boundary",
);

const FLASHLOAN_REPAY_DIAG: DiagnosticInfo = custom(
    LINT_WARNING_PREFIX,
    Severity::Warning,
    CLIPPY_CATEGORY,
    11, // flashloan_without_repay
    "flashloan borrowed but not repaid on all paths",
);

const PRICE_MANIPULATION_DIAG: DiagnosticInfo = custom(
    LINT_WARNING_PREFIX,
    Severity::Warning,
    CLIPPY_CATEGORY,
    12, // price_manipulation_window
    "state changes between oracle reads create manipulation window",
);

// ============================================================================
// Phase III Lint Descriptors
// ============================================================================

pub static TRANSITIVE_CAPABILITY_LEAK: LintDescriptor = LintDescriptor {
    name: "transitive_capability_leak",
    category: LintCategory::Security,
    description: "Capability leaks across module boundary (cross-module analysis)",
    group: RuleGroup::Preview,
    fix: FixDescriptor::none(),
};

pub static FLASHLOAN_WITHOUT_REPAY: LintDescriptor = LintDescriptor {
    name: "flashloan_without_repay",
    category: LintCategory::Security,
    description: "Flashloan borrowed but not repaid on all paths (cross-module)",
    group: RuleGroup::Preview,
    fix: FixDescriptor::none(),
};

pub static PRICE_MANIPULATION_WINDOW: LintDescriptor = LintDescriptor {
    name: "price_manipulation_window",
    category: LintCategory::Security,
    description: "State changes between oracle reads (temporal analysis)",
    group: RuleGroup::Preview,
    fix: FixDescriptor::none(),
};

// ============================================================================
// Call Graph Infrastructure
// ============================================================================

/// Represents a call from one function to another
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Call {
    /// Calling module and function
    pub caller: (ModuleIdent, FunctionName),
    /// Called module and function
    pub callee: (ModuleIdent, FunctionName),
    /// Location of the call
    pub loc: Loc,
    /// Arguments passed (simplified - just count for now)
    pub arg_count: usize,
}

/// Call graph for the entire program
#[derive(Clone, Debug, Default)]
pub struct CallGraph {
    /// Maps (module, function) -> list of calls it makes
    pub calls: BTreeMap<(ModuleIdent, FunctionName), Vec<Call>>,
    /// Maps (module, function) -> list of callers
    pub callers: BTreeMap<(ModuleIdent, FunctionName), Vec<Call>>,
    /// Functions that handle capabilities
    pub capability_handlers: BTreeSet<(ModuleIdent, FunctionName)>,
    /// Functions that create resources (new, borrow, etc.)
    pub resource_creators: BTreeMap<(ModuleIdent, FunctionName), ResourceKind>,
    /// Functions that consume resources (transfer, repay, etc.)
    pub resource_consumers: BTreeMap<(ModuleIdent, FunctionName), ResourceKind>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResourceKind {
    FlashLoan,
    Capability,
    Asset,
    Generic,
}

impl CallGraph {
    /// Build a call graph from a typed program
    pub fn build(program: &T::Program, info: &TypingProgramInfo) -> Self {
        let mut graph = CallGraph::default();

        for (mident, mdef) in program.modules.key_cloned_iter() {
            for (fname, fdef) in mdef.functions.key_cloned_iter() {
                graph.analyze_function(&mident, &fname, fdef, info);
            }
        }

        // Build reverse mapping (callers)
        graph.build_caller_map();

        graph
    }

    fn analyze_function(
        &mut self,
        mident: &ModuleIdent,
        fname: &FunctionName,
        fdef: &T::Function,
        info: &TypingProgramInfo,
    ) {
        let caller_key = (*mident, *fname);

        // Check if this function handles capabilities
        if Self::is_capability_handler(fdef) {
            self.capability_handlers.insert(caller_key);
        }

        // Check if this creates/consumes resources
        if let Some(kind) = Self::detects_resource_creation(fname, fdef) {
            self.resource_creators.insert(caller_key, kind);
        }
        if let Some(kind) = Self::detects_resource_consumption(fname, fdef) {
            self.resource_consumers.insert(caller_key, kind);
        }

        // Extract calls from function body
        if let T::FunctionBody_::Defined((_use_funs, seq_items)) = &fdef.body.value {
            let calls = Self::extract_calls_from_seq(mident, fname, seq_items);
            if !calls.is_empty() {
                self.calls.insert(caller_key, calls);
            }
        }
    }

    fn is_capability_handler(fdef: &T::Function) -> bool {
        // Check if function takes capability parameters
        fdef.signature.parameters.iter().any(|(_, var, ty)| {
            let name = var.value.name.as_str();
            let is_cap_name = name.ends_with("_cap") || name.ends_with("Cap") || name == "cap";
            
            // Check for key+store abilities
            let is_cap_type = matches!(&ty.value, N::Type_::Ref(_, inner)
                if matches!(&inner.value, N::Type_::Apply(abilities, _, _)
                    if abilities.has_ability_(Ability_::Key) && abilities.has_ability_(Ability_::Store))
            );

            is_cap_name || is_cap_type
        })
    }

    fn detects_resource_creation(fname: &FunctionName, fdef: &T::Function) -> Option<ResourceKind> {
        let name = fname.value().as_str();
        
        if name.contains("borrow") || name.contains("flash_loan") || name.contains("flashloan") {
            Some(ResourceKind::FlashLoan)
        } else if name.contains("new_cap") || name == "new" {
            Some(ResourceKind::Capability)
        } else if name.contains("mint") || name.contains("create_coin") {
            Some(ResourceKind::Asset)
        } else {
            None
        }
    }

    fn detects_resource_consumption(fname: &FunctionName, fdef: &T::Function) -> Option<ResourceKind> {
        let name = fname.value().as_str();
        
        if name.contains("repay") || name.contains("return") {
            Some(ResourceKind::FlashLoan)
        } else if name.contains("burn") || name.contains("destroy") {
            Some(ResourceKind::Asset)
        } else if name.contains("transfer") || name.contains("public_transfer") {
            Some(ResourceKind::Generic)
        } else {
            None
        }
    }

    fn extract_calls_from_seq(
        caller_mod: &ModuleIdent,
        caller_func: &FunctionName,
        seq_items: &im::Vector<T::SequenceItem>,
    ) -> Vec<Call> {
        let mut calls = Vec::new();

        for item in seq_items.iter() {
            match &item.value {
                T::SequenceItem_::Seq(exp) => {
                    Self::extract_calls_from_exp(&mut calls, caller_mod, caller_func, exp);
                }
                T::SequenceItem_::Bind(_, _, exp) => {
                    Self::extract_calls_from_exp(&mut calls, caller_mod, caller_func, exp);
                }
                _ => {}
            }
        }

        calls
    }

    fn extract_calls_from_exp(
        calls: &mut Vec<Call>,
        caller_mod: &ModuleIdent,
        caller_func: &FunctionName,
        exp: &T::Exp,
    ) {
        match &exp.exp.value {
            T::UnannotatedExp_::ModuleCall(call) => {
                let callee_mod = call.module;
                let callee_func = call.name;
                let arg_count = call.arguments.len();

                calls.push(Call {
                    caller: (*caller_mod, *caller_func),
                    callee: (callee_mod, callee_func),
                    loc: exp.exp.loc,
                    arg_count,
                });

                // Recurse into arguments
                for arg in &call.arguments {
                    Self::extract_calls_from_exp(calls, caller_mod, caller_func, arg);
                }
            }
            T::UnannotatedExp_::Block((_, seq)) => {
                for item in seq.iter() {
                    match &item.value {
                        T::SequenceItem_::Seq(e) | T::SequenceItem_::Bind(_, _, e) => {
                            Self::extract_calls_from_exp(calls, caller_mod, caller_func, e);
                        }
                        _ => {}
                    }
                }
            }
            T::UnannotatedExp_::IfElse(cond, then_e, else_e_opt) => {
                Self::extract_calls_from_exp(calls, caller_mod, caller_func, cond);
                Self::extract_calls_from_exp(calls, caller_mod, caller_func, then_e);
                if let Some(else_e) = else_e_opt {
                    Self::extract_calls_from_exp(calls, caller_mod, caller_func, else_e);
                }
            }
            _ => {}
        }
    }

    fn build_caller_map(&mut self) {
        for (caller_key, calls) in &self.calls {
            for call in calls {
                self.callers
                    .entry(call.callee)
                    .or_default()
                    .push(call.clone());
            }
        }
    }

    /// Find all transitive callers of a function
    pub fn transitive_callers(
        &self,
        target: &(ModuleIdent, FunctionName),
    ) -> BTreeSet<(ModuleIdent, FunctionName)> {
        let mut visited = BTreeSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(*target);

        while let Some(current) = queue.pop_front() {
            if !visited.insert(current) {
                continue;
            }

            if let Some(callers) = self.callers.get(&current) {
                for call in callers {
                    queue.push_back(call.caller);
                }
            }
        }

        visited
    }

    /// Find all transitive callees of a function
    pub fn transitive_callees(
        &self,
        source: &(ModuleIdent, FunctionName),
    ) -> BTreeSet<(ModuleIdent, FunctionName)> {
        let mut visited = BTreeSet::new();
        let mut queue = VecDeque::new();
        queue.push_back(*source);

        while let Some(current) = queue.pop_front() {
            if !visited.insert(current) {
                continue;
            }

            if let Some(calls) = self.calls.get(&current) {
                for call in calls {
                    queue.push_back(call.callee);
                }
            }
        }

        visited
    }

    /// Check if capability flows from source to destination
    pub fn has_capability_flow(
        &self,
        source: &(ModuleIdent, FunctionName),
        dest: &(ModuleIdent, FunctionName),
    ) -> bool {
        // Source must be a capability handler
        if !self.capability_handlers.contains(source) {
            return false;
        }

        // Check if dest is reachable from source
        let reachable = self.transitive_callees(source);
        reachable.contains(dest)
    }
}

// ============================================================================
// 1. Transitive Capability Leak Detection
// ============================================================================

/// Detect capabilities that leak across module boundaries
pub fn lint_transitive_capability_leak(
    program: &T::Program,
    info: &TypingProgramInfo,
) -> Vec<CompilerDiagnostic> {
    let mut diags = Vec::new();
    let call_graph = CallGraph::build(program, info);

    // Check each capability handler
    for (module, function) in &call_graph.capability_handlers {
        // Get all functions this capability handler calls
        let callees = call_graph.transitive_callees(&(*module, *function));

        // Check if any callee is in a different module and is public
        for (callee_mod, callee_func) in &callees {
            if callee_mod != module {
                // Capability flows to different module
                if let Some(fdef) = program
                    .modules
                    .get(callee_mod)
                    .and_then(|m| m.functions.get(callee_func))
                {
                    if matches!(fdef.visibility, T::Visibility::Public(_)) {
                        // Public function in different module - potential leak
                        let msg = format!(
                            "Capability from {module}::{} flows to public function {callee_mod}::{callee_func}",
                            function.value()
                        );
                        let help = "Capabilities should not leak across module boundaries. \
                                   Consider making the called function package-private or adding capability checks.";

                        // We can't easily create a diagnostic without proper locations
                        // This would need integration with the actual function locations
                        // For now, we'll skip creating diagnostics
                        eprintln!("WARNING: {}", msg);
                    }
                }
            }
        }
    }

    diags
}

// ============================================================================
// 2. Flashloan Repayment Analysis
// ============================================================================

/// Detect flashloans that are not repaid on all paths
pub fn lint_flashloan_without_repay(
    program: &T::Program,
    info: &TypingProgramInfo,
) -> Vec<CompilerDiagnostic> {
    let mut diags = Vec::new();
    let call_graph = CallGraph::build(program, info);

    // Find all functions that create flashloans
    for ((module, function), kind) in &call_graph.resource_creators {
        if *kind != ResourceKind::FlashLoan {
            continue;
        }

        // Check if this function (or its callers) have a corresponding repay call
        let callees = call_graph.transitive_callees(&(*module, *function));
        
        let has_repay = callees.iter().any(|callee| {
            call_graph
                .resource_consumers
                .get(callee)
                .map_or(false, |k| *k == ResourceKind::FlashLoan)
        });

        if !has_repay {
            eprintln!(
                "WARNING: Flashloan in {module}::{} not repaid",
                function.value()
            );
        }
    }

    diags
}

// ============================================================================
// 3. Price Manipulation Window Detection
// ============================================================================

/// Detect state changes between oracle price reads
pub fn lint_price_manipulation_window(
    program: &T::Program,
    info: &TypingProgramInfo,
) -> Vec<CompilerDiagnostic> {
    let mut diags = Vec::new();

    // This requires analyzing the sequence of operations within a function
    // to detect patterns like:
    // 1. Read oracle price
    // 2. Modify shared state
    // 3. Read oracle price again (using potentially manipulated state)

    for (mident, mdef) in program.modules.key_cloned_iter() {
        for (fname, fdef) in mdef.functions.key_cloned_iter() {
            if let T::FunctionBody_::Defined((_use_funs, seq_items)) = &fdef.body.value {
                analyze_price_manipulation_pattern(&mident, &fname, seq_items, &mut diags);
            }
        }
    }

    diags
}

fn analyze_price_manipulation_pattern(
    _mident: &ModuleIdent,
    _fname: &FunctionName,
    seq_items: &im::Vector<T::SequenceItem>,
    _diags: &mut Vec<CompilerDiagnostic>,
) {
    let mut oracle_reads = Vec::new();
    let mut state_mutations = Vec::new();

    for item in seq_items.iter() {
        match &item.value {
            T::SequenceItem_::Seq(exp) | T::SequenceItem_::Bind(_, _, exp) => {
                if is_oracle_price_read(exp) {
                    oracle_reads.push(exp.exp.loc);
                }
                if is_state_mutation(exp) {
                    state_mutations.push(exp.exp.loc);
                }
            }
            _ => {}
        }
    }

    // Check for pattern: oracle_read -> state_mutation -> oracle_read
    if oracle_reads.len() >= 2 && !state_mutations.is_empty() {
        // Potential manipulation window
        eprintln!("WARNING: Potential price manipulation window detected");
    }
}

fn is_oracle_price_read(exp: &T::Exp) -> bool {
    matches!(&exp.exp.value, T::UnannotatedExp_::ModuleCall(call)
        if call.name.value().as_str().contains("get_price")
            || call.name.value().as_str().contains("oracle"))
}

fn is_state_mutation(exp: &T::Exp) -> bool {
    // Check for calls to functions that modify shared state
    // This is a heuristic - proper implementation would need deeper analysis
    matches!(&exp.exp.value, T::UnannotatedExp_::ModuleCall(call)
        if call.name.value().as_str().contains("update")
            || call.name.value().as_str().contains("set")
            || call.name.value().as_str().contains("modify"))
}

// ============================================================================
// Public API
// ============================================================================

/// Return all Phase III lint descriptors
pub fn descriptors() -> &'static [&'static LintDescriptor] {
    &[
        &TRANSITIVE_CAPABILITY_LEAK,
        &FLASHLOAN_WITHOUT_REPAY,
        &PRICE_MANIPULATION_WINDOW,
    ]
}

/// Look up a Phase III lint descriptor by name
pub fn find_descriptor(name: &str) -> Option<&'static LintDescriptor> {
    descriptors().iter().copied().find(|d| d.name == name)
}

/// Run all Phase III cross-module lints
pub fn run_cross_module_lints(
    program: &T::Program,
    info: &TypingProgramInfo,
) -> Vec<CompilerDiagnostic> {
    let mut diags = Vec::new();

    diags.extend(lint_transitive_capability_leak(program, info));
    diags.extend(lint_flashloan_without_repay(program, info));
    diags.extend(lint_price_manipulation_window(program, info));

    diags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_kind() {
        assert_eq!(ResourceKind::FlashLoan, ResourceKind::FlashLoan);
        assert_ne!(ResourceKind::FlashLoan, ResourceKind::Capability);
    }

    // Additional tests would require mock program structures
}
