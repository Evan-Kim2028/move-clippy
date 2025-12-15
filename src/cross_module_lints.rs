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

#![allow(unused)]

use crate::diagnostics::Diagnostic;
use crate::error::ClippyResult;
use crate::lint::{
    AnalysisKind, FixDescriptor, LintCategory, LintDescriptor, LintSettings, RuleGroup,
};
use move_compiler::{
    diag,
    diagnostics::{
        Diagnostic as CompilerDiagnostic, Diagnostics as CompilerDiagnostics,
        codes::{DiagnosticInfo, Severity, custom},
    },
    expansion::ast::{ModuleIdent, Visibility},
    hlir::ast::{
        BaseType, BaseType_, Exp, ModuleCall, SingleType, SingleType_, Type, Type_,
        UnannotatedExp_, Var,
    },
    naming::ast as N,
    parser::ast::{Ability_, DatatypeName, FunctionName, TargetKind},
    shared::{Identifier, program_info::TypingProgramInfo},
    typing::ast as T,
};
use move_ir_types::location::*;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

// ============================================================================
// Lint Diagnostic Codes
// ============================================================================

const LINT_WARNING_PREFIX: &str = "Lint";
const CLIPPY_CATEGORY: u8 = 50; // Must be <= 99

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

// NOTE: PRICE_MANIPULATION_DIAG removed - price_manipulation_window used name-based heuristics

// ============================================================================
// Phase III Lint Descriptors (cross-module call graph analysis)
// ============================================================================

pub static TRANSITIVE_CAPABILITY_LEAK: LintDescriptor = LintDescriptor {
    name: "transitive_capability_leak",
    category: LintCategory::Security,
    description: "Capability leaks across module boundary (type-based cross-module analysis)",
    group: RuleGroup::Preview,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::CrossModule,
};

pub static FLASHLOAN_WITHOUT_REPAY: LintDescriptor = LintDescriptor {
    name: "flashloan_without_repay",
    category: LintCategory::Security,
    description: "Flashloan borrowed but not repaid on all paths (type-based cross-module)",
    group: RuleGroup::Preview,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::CrossModule,
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

fn is_hot_potato_param(param_ty: &N::Type_) -> bool {
    // Hot potato: by-value parameter with no drop ability.
    // We conservatively treat any by-value type lacking `drop` as a hot potato.
    matches!(
        param_ty,
        N::Type_::Apply(Some(abilities), _tname, _tys) if !abilities.has_ability_(Ability_::Drop)
    )
}

fn is_hot_potato_return(ret_ty: &N::Type_) -> bool {
    matches!(
        ret_ty,
        N::Type_::Apply(Some(abilities), _tname, _tys) if !abilities.has_ability_(Ability_::Drop)
    )
}

fn has_hot_potato_by_value_param(fdef: &T::Function) -> bool {
    fdef.signature
        .parameters
        .iter()
        .any(|(_mut_, _var, ty)| is_hot_potato_param(&ty.value))
}

fn root_package_modules(program: &T::Program) -> BTreeSet<ModuleIdent> {
    program
        .modules
        .key_cloned_iter()
        .filter_map(|(mident, mdef)| match mdef.target_kind {
            TargetKind::Source {
                is_root_package: true,
            } => Some(mident),
            _ => None,
        })
        .collect()
}

fn is_root_package_module(root_modules: &BTreeSet<ModuleIdent>, mident: &ModuleIdent) -> bool {
    root_modules.contains(mident)
}

impl CallGraph {
    /// Build a call graph from a typed program.
    ///
    /// If `root_modules` is provided, the graph is restricted to the package boundary:
    /// - Only functions in `root_modules` are analyzed
    /// - Only call edges where both caller and callee are in `root_modules` are recorded
    pub fn build(program: &T::Program, info: &TypingProgramInfo) -> Self {
        Self::build_scoped(program, info, None)
    }

    pub fn build_scoped(
        program: &T::Program,
        info: &TypingProgramInfo,
        root_modules: Option<&BTreeSet<ModuleIdent>>,
    ) -> Self {
        let mut graph = CallGraph::default();

        for (mident, mdef) in program.modules.key_cloned_iter() {
            if let Some(roots) = root_modules
                && !is_root_package_module(roots, &mident)
            {
                continue;
            }

            for (fname, fdef) in mdef.functions.key_cloned_iter() {
                graph.analyze_function(&mident, &fname, fdef, info, root_modules);
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
        root_modules: Option<&BTreeSet<ModuleIdent>>,
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
            let calls = Self::extract_calls_from_seq(mident, fname, seq_items, root_modules);
            if !calls.is_empty() {
                self.calls.insert(caller_key, calls);
            }
        }
    }

    fn is_capability_handler(_fdef: &T::Function) -> bool {
        // Deprecated: capability handler classification was too heuristic.
        // Capability leakage is now detected by analyzing actual by-value argument flows.
        false
    }

    fn detects_resource_creation(
        _fname: &FunctionName,
        fdef: &T::Function,
    ) -> Option<ResourceKind> {
        // Scope creators to "flashloan-like" behavior: takes a hot potato by value and returns a hot potato.
        // This avoids flagging generic constructors (e.g. `object::new`) that return no-drop values.
        if is_hot_potato_return(&fdef.signature.return_type.value)
            && has_hot_potato_by_value_param(fdef)
        {
            return Some(ResourceKind::FlashLoan);
        }

        // Capability return type: key+store.
        if let N::Type_::Apply(Some(abilities), _, _) = &fdef.signature.return_type.value
            && abilities.has_ability_(Ability_::Key)
            && abilities.has_ability_(Ability_::Store)
        {
            return Some(ResourceKind::Capability);
        }

        None
    }

    fn detects_resource_consumption(
        _fname: &FunctionName,
        fdef: &T::Function,
    ) -> Option<ResourceKind> {
        // A function that takes a hot potato by value is considered a consumer.
        if has_hot_potato_by_value_param(fdef) {
            return Some(ResourceKind::FlashLoan);
        }

        None
    }

    fn extract_calls_from_seq(
        caller_mod: &ModuleIdent,
        caller_func: &FunctionName,
        seq_items: &VecDeque<T::SequenceItem>,
        root_modules: Option<&BTreeSet<ModuleIdent>>,
    ) -> Vec<Call> {
        let mut calls = Vec::new();

        for item in seq_items.iter() {
            match &item.value {
                T::SequenceItem_::Seq(exp) => {
                    Self::extract_calls_from_exp(
                        &mut calls,
                        caller_mod,
                        caller_func,
                        exp,
                        root_modules,
                    );
                }
                T::SequenceItem_::Bind(_, _, exp) => {
                    Self::extract_calls_from_exp(
                        &mut calls,
                        caller_mod,
                        caller_func,
                        exp,
                        root_modules,
                    );
                }
                _ => {}
            }
        }

        calls
    }

    fn record_call_if_in_scope(
        calls: &mut Vec<Call>,
        caller_mod: &ModuleIdent,
        caller_func: &FunctionName,
        callee_mod: &ModuleIdent,
        callee_func: &FunctionName,
        loc: Loc,
        root_modules: Option<&BTreeSet<ModuleIdent>>,
    ) {
        if let Some(roots) = root_modules
            && (!is_root_package_module(roots, caller_mod)
                || !is_root_package_module(roots, callee_mod))
        {
            return;
        }

        calls.push(Call {
            caller: (*caller_mod, *caller_func),
            callee: (*callee_mod, *callee_func),
            loc,
        });
    }

    fn extract_calls_from_exp(
        calls: &mut Vec<Call>,
        caller_mod: &ModuleIdent,
        caller_func: &FunctionName,
        exp: &T::Exp,
        root_modules: Option<&BTreeSet<ModuleIdent>>,
    ) {
        match &exp.exp.value {
            T::UnannotatedExp_::ModuleCall(call) => {
                Self::record_call_if_in_scope(
                    calls,
                    caller_mod,
                    caller_func,
                    &call.module,
                    &call.name,
                    exp.exp.loc,
                    root_modules,
                );

                // Recurse into arguments (Box<Exp>)
                Self::extract_calls_from_exp(
                    calls,
                    caller_mod,
                    caller_func,
                    &call.arguments,
                    root_modules,
                );
            }

            // Common recursive expression forms
            T::UnannotatedExp_::IfElse(cond, then_e, else_e_opt) => {
                Self::extract_calls_from_exp(calls, caller_mod, caller_func, cond, root_modules);
                Self::extract_calls_from_exp(calls, caller_mod, caller_func, then_e, root_modules);
                if let Some(else_e) = else_e_opt {
                    Self::extract_calls_from_exp(
                        calls,
                        caller_mod,
                        caller_func,
                        else_e,
                        root_modules,
                    );
                }
            }
            T::UnannotatedExp_::While(_, cond, body) => {
                Self::extract_calls_from_exp(calls, caller_mod, caller_func, cond, root_modules);
                Self::extract_calls_from_exp(calls, caller_mod, caller_func, body, root_modules);
            }
            T::UnannotatedExp_::Loop { body, .. } => {
                Self::extract_calls_from_exp(calls, caller_mod, caller_func, body, root_modules);
            }
            T::UnannotatedExp_::Block((_, seq)) => {
                for item in seq.iter() {
                    match &item.value {
                        T::SequenceItem_::Seq(e) => {
                            Self::extract_calls_from_exp(
                                calls,
                                caller_mod,
                                caller_func,
                                e,
                                root_modules,
                            );
                        }
                        T::SequenceItem_::Bind(_, _, e) => {
                            Self::extract_calls_from_exp(
                                calls,
                                caller_mod,
                                caller_func,
                                e,
                                root_modules,
                            );
                        }
                        _ => {}
                    }
                }
            }
            T::UnannotatedExp_::NamedBlock(_, (_, seq)) => {
                for item in seq.iter() {
                    match &item.value {
                        T::SequenceItem_::Seq(e) => {
                            Self::extract_calls_from_exp(
                                calls,
                                caller_mod,
                                caller_func,
                                e,
                                root_modules,
                            );
                        }
                        T::SequenceItem_::Bind(_, _, e) => {
                            Self::extract_calls_from_exp(
                                calls,
                                caller_mod,
                                caller_func,
                                e,
                                root_modules,
                            );
                        }
                        _ => {}
                    }
                }
            }
            T::UnannotatedExp_::BinopExp(left, _, _, right) => {
                Self::extract_calls_from_exp(calls, caller_mod, caller_func, left, root_modules);
                Self::extract_calls_from_exp(calls, caller_mod, caller_func, right, root_modules);
            }
            T::UnannotatedExp_::UnaryExp(_, inner) => {
                Self::extract_calls_from_exp(calls, caller_mod, caller_func, inner, root_modules);
            }
            T::UnannotatedExp_::Assign(_, _, rhs) => {
                Self::extract_calls_from_exp(calls, caller_mod, caller_func, rhs, root_modules);
            }
            T::UnannotatedExp_::Mutate(lhs, rhs) => {
                Self::extract_calls_from_exp(calls, caller_mod, caller_func, lhs, root_modules);
                Self::extract_calls_from_exp(calls, caller_mod, caller_func, rhs, root_modules);
            }
            T::UnannotatedExp_::Return(inner)
            | T::UnannotatedExp_::Abort(inner)
            | T::UnannotatedExp_::Give(_, inner)
            | T::UnannotatedExp_::Cast(inner, _)
            | T::UnannotatedExp_::Annotate(inner, _)
            | T::UnannotatedExp_::Dereference(inner)
            | T::UnannotatedExp_::Borrow(_, inner, _)
            | T::UnannotatedExp_::TempBorrow(_, inner) => {
                Self::extract_calls_from_exp(calls, caller_mod, caller_func, inner, root_modules);
            }
            T::UnannotatedExp_::ExpList(items) => {
                for item in items {
                    match item {
                        T::ExpListItem::Single(e, _) | T::ExpListItem::Splat(_, e, _) => {
                            Self::extract_calls_from_exp(
                                calls,
                                caller_mod,
                                caller_func,
                                e,
                                root_modules,
                            );
                        }
                    }
                }
            }
            T::UnannotatedExp_::Pack(_, _, _, fields) => {
                for (_, _, (_, (_, e))) in fields {
                    Self::extract_calls_from_exp(calls, caller_mod, caller_func, e, root_modules);
                }
            }
            T::UnannotatedExp_::PackVariant(_, _, _, _, fields) => {
                for (_, _, (_, (_, e))) in fields {
                    Self::extract_calls_from_exp(calls, caller_mod, caller_func, e, root_modules);
                }
            }
            T::UnannotatedExp_::Match(scrutinee, arms) => {
                Self::extract_calls_from_exp(
                    calls,
                    caller_mod,
                    caller_func,
                    scrutinee,
                    root_modules,
                );
                for arm in &arms.value {
                    Self::extract_calls_from_exp(
                        calls,
                        caller_mod,
                        caller_func,
                        &arm.value.rhs,
                        root_modules,
                    );
                }
            }
            T::UnannotatedExp_::VariantMatch(scrutinee, _, arms) => {
                Self::extract_calls_from_exp(
                    calls,
                    caller_mod,
                    caller_func,
                    scrutinee,
                    root_modules,
                );
                for (_, rhs) in arms {
                    Self::extract_calls_from_exp(calls, caller_mod, caller_func, rhs, root_modules);
                }
            }

            // Base cases (no recursion needed)
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

    let root_modules = root_package_modules(program);
    let call_graph = CallGraph::build_scoped(program, info, Some(&root_modules));

    for (caller, calls) in &call_graph.calls {
        let (caller_mod, caller_func) = caller;
        if !is_root_package_module(&root_modules, caller_mod) {
            continue;
        }

        let caller_symbol = caller_func.value();
        let caller_name = caller_symbol.as_str();

        for call in calls {
            let (callee_mod, callee_func) = call.callee;
            if callee_mod == *caller_mod {
                continue;
            }
            if !is_root_package_module(&root_modules, &callee_mod) {
                continue;
            }

            let Some(callee_mdef) = program.modules.get(&callee_mod) else {
                continue;
            };
            let Some(callee_fdef) = callee_mdef.functions.get(&callee_func) else {
                continue;
            };
            if !matches!(callee_fdef.visibility, Visibility::Public(_)) {
                continue;
            }

            // If the callee has any by-value key+store parameters, a cross-module call
            // could leak a capability value into a public API.
            let mut expects_cap_by_value = false;
            for (_mut_, _param_var, param_ty) in &callee_fdef.signature.parameters {
                if matches!(
                    &param_ty.value,
                    N::Type_::Apply(Some(abilities), _, _)
                        if abilities.has_ability_(Ability_::Key) && abilities.has_ability_(Ability_::Store)
                ) {
                    expects_cap_by_value = true;
                    break;
                }
            }
            if !expects_cap_by_value {
                continue;
            }

            let callee_symbol = callee_func.value();
            let callee_name = callee_symbol.as_str();
            let msg = format!(
                "Capability value may leak from {caller_mod}::{caller_name} to public function {callee_mod}::{callee_name}"
            );
            let help = "Capability values should not cross module boundaries into public APIs. \
                       Consider passing by reference, restricting visibility, or moving checks into the callee.";

            diags.push(diag!(
                TRANSITIVE_CAP_LEAK_DIAG,
                (call.loc, msg),
                (
                    callee_fdef.loc,
                    "Public callee has key+store by-value param"
                ),
                (call.loc, help),
            ));
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

    let root_modules = root_package_modules(program);
    let call_graph = CallGraph::build_scoped(program, info, Some(&root_modules));

    // Creator: returns hot potato (no drop). Consumer: takes hot potato by value.
    for ((module, function), kind) in &call_graph.resource_creators {
        if *kind != ResourceKind::FlashLoan {
            continue;
        }
        if !is_root_package_module(&root_modules, module) {
            continue;
        }

        // If there is no consumer anywhere in the program, this lint is too noisy.
        // This keeps the lint conservative until we have a FunctionSummary-based implementation.
        if call_graph
            .resource_consumers
            .values()
            .all(|k| *k != ResourceKind::FlashLoan)
        {
            continue;
        }

        let Some(mdef) = program.modules.get(module) else {
            continue;
        };
        let Some(fdef) = mdef.functions.get(function) else {
            continue;
        };
        let func_loc = fdef.loc;

        // If the function (or anything it transitively calls) consumes a hot potato by value,
        // we treat it as "repaid".
        let callees = call_graph.transitive_callees(&(*module, *function));
        let has_consume = callees.iter().any(|callee| {
            if !is_root_package_module(&root_modules, &callee.0) {
                return false;
            }
            call_graph
                .resource_consumers
                .get(callee)
                .is_some_and(|k| *k == ResourceKind::FlashLoan)
        });

        if !has_consume {
            let func_symbol = function.value();
            let msg = format!(
                "Flashloan/hot-potato returned by {module}::{} is not consumed on all code paths",
                func_symbol.as_str()
            );
            let help = "Values without `drop` ability must be consumed (e.g. repaid/destroyed) before function exit; ensure all paths consume it.";

            diags.push(diag!(
                FLASHLOAN_REPAY_DIAG,
                (func_loc, msg),
                (func_loc, help)
            ));
        }
    }

    diags
}

// ============================================================================
// 3. Price Manipulation Window Detection
// ============================================================================
// 3. Price Manipulation Window Detection - REMOVED
// ============================================================================
// NOTE: lint_price_manipulation_window and related functions removed - used
// name-based heuristics (checking function names like "get_price", "oracle",
// "update", "set") rather than true type-based detection. A proper implementation
// would require:
// - Type-based oracle identification (not name-based)
// - Proper state mutation tracking through type effects
// - Integration with actual oracle type definitions

fn is_key_store_base_type(bt: &BaseType_) -> bool {
    // TODO(infra): Reuse `crate::type_classifier`-style predicates for ability checks across modules.
    matches!(
        bt,
        BaseType_::Apply(abilities, _, _)
            if abilities.has_ability_(Ability_::Key) && abilities.has_ability_(Ability_::Store)
    )
}

fn single_type_is_key_store_value(st: &SingleType_) -> bool {
    match st {
        // By-value capability
        SingleType_::Base(bt) => is_key_store_base_type(&bt.value),
        // References are not leaks
        SingleType_::Ref(_, _bt) => false,
    }
}

fn type_is_key_store_value(ty: &Type_) -> bool {
    match ty {
        Type_::Single(st) => single_type_is_key_store_value(&st.value),
        // We conservatively ignore tuples here; leak detection is targeted.
        _ => false,
    }
}

// ============================================================================
// Public API
// ============================================================================

// Static slice for descriptors (avoids returning reference to temporary)
// NOTE: PRICE_MANIPULATION_WINDOW removed - used name-based heuristics
static DESCRIPTORS: &[&LintDescriptor] = &[&TRANSITIVE_CAPABILITY_LEAK, &FLASHLOAN_WITHOUT_REPAY];

/// Return all Phase III lint descriptors
pub fn descriptors() -> &'static [&'static LintDescriptor] {
    DESCRIPTORS
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
    // NOTE: lint_price_manipulation_window removed - used name-based heuristics

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
