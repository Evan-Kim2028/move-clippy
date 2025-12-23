// Phase II: SimpleAbsInt-based Security Lints
//
// This module implements control-flow-aware security lints using the Sui Move
// compiler's abstract interpretation framework (SimpleAbsInt).
//
// Architecture based on: sui/external-crates/move/crates/move-compiler/src/sui_mode/linters/share_owned.rs
//
// References:
// - share_owned.rs: Template for abstract interpretation patterns
// - cfgir/visitor.rs: SimpleAbsInt trait definitions
// - cfgir/absint.rs: Abstract domain and transfer functions
//
// Phase 1 & 2 Implementation:
// - Rich 4-state value lattice: Unused → AccessedNotValidated → PendingValidation → Validated
// - Type-based privileged sink detection (replaces name-based heuristics)
// - Guard pattern detection (JumpIf with abort branch recognition)
// - Validation state tracking that survives Move operations

#![allow(unused)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::collapsible_match)]
#![allow(clippy::only_used_in_recursion)]
#![allow(clippy::question_mark)]
#![allow(clippy::unwrap_or_default)]

use crate::diagnostics::Diagnostic;
use crate::error::Result as ClippyResult;
use crate::lint::{
    AnalysisKind, FixDescriptor, LintCategory, LintDescriptor, LintSettings, RuleGroup,
    TypeSystemGap,
};
use move_compiler::shared::NumericalAddress;
use move_compiler::{
    PreCompiledProgramInfo,
    cfgir::{
        CFGContext,
        absint::JoinResult,
        cfg::{CFG, ImmForwardCFG},
        visitor::{
            AbstractInterpreterVisitor, LocalState, SimpleAbsInt, SimpleAbsIntConstructor,
            SimpleDomain, SimpleExecutionContext,
        },
    },
    diag,
    diagnostics::{
        Diagnostic as CompilerDiagnostic, Diagnostics as CompilerDiagnostics,
        codes::{DiagnosticInfo, Severity, custom},
    },
    expansion::ast::ModuleIdent,
    hlir::ast::{
        BaseType, BaseType_, Command, Command_, Exp, LValue, LValue_, Label, ModuleCall,
        SingleType, SingleType_, StructFields, Type, Type_, TypeName, TypeName_, UnannotatedExp_,
        Value as HValue, Value_, Var,
    },
    naming::ast::{
        BuiltinTypeName_, StructFields as NStructFields, Type as NType, Type_ as NType_,
    },
    parser::ast::{Ability_, BinOp_, DatatypeName},
    shared::{Identifier, program_info::TypingProgramInfo},
};
use move_ir_types::location::*;
use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet},
    ptr::NonNull,
    sync::Arc,
};

// ============================================================================
// Constants
// ============================================================================

/// Sui framework address (0x2)
const SUI_ADDR_NAME: &str = "sui";
const SUI_ADDR: NumericalAddress = NumericalAddress::new(
    [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 2,
    ],
    move_compiler::shared::NumberFormat::Hex,
);

// ============================================================================
// Lint Diagnostic Codes
// ============================================================================

const LINT_WARNING_PREFIX: &str = "Lint";
const CLIPPY_CATEGORY: u8 = 50; // Custom category for move-clippy (must be <= 99)

const UNUSED_CAP_V2_DIAG: DiagnosticInfo = custom(
    LINT_WARNING_PREFIX,
    Severity::Warning,
    CLIPPY_CATEGORY,
    1, // phantom_capability
    "capability parameter is unused",
);

const UNVALIDATED_CAP_V2_DIAG: DiagnosticInfo = custom(
    LINT_WARNING_PREFIX,
    Severity::Warning,
    CLIPPY_CATEGORY,
    3, // unvalidated_capability_param
    "capability parameter accessed but not validated",
);

const UNCHECKED_DIV_V2_DIAG: DiagnosticInfo = custom(
    LINT_WARNING_PREFIX,
    Severity::Warning,
    CLIPPY_CATEGORY,
    2, // unchecked_division_v2
    "division without zero-check validation",
);

const STALE_ORACLE_PRICE_V3_DIAG: DiagnosticInfo = custom(
    LINT_WARNING_PREFIX,
    Severity::Warning,
    CLIPPY_CATEGORY,
    8, // stale_oracle_price_v3
    "oracle price used without freshness validation",
);

// ============================================================================
// Phase II Lint Descriptors (type-based with CFG analysis)
// ============================================================================

pub static PHANTOM_CAPABILITY: LintDescriptor = LintDescriptor {
    name: "phantom_capability",
    category: LintCategory::Security,
    description: "Capability parameter unused or not validated - may be phantom security (type-based CFG-aware, requires --mode full --experimental)",
    group: RuleGroup::Experimental,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBasedCFG,
    gap: Some(TypeSystemGap::CapabilityEscape),
};

pub static UNCHECKED_DIVISION_V2: LintDescriptor = LintDescriptor {
    name: "unchecked_division_v2",
    category: LintCategory::Security,
    description: "Division without zero-check (type-based CFG-aware, requires --mode full --preview)",
    group: RuleGroup::Preview,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBasedCFG,
    gap: Some(TypeSystemGap::ArithmeticSafety),
};

pub static STALE_ORACLE_PRICE_V3: LintDescriptor = LintDescriptor {
    name: "stale_oracle_price_v3",
    category: LintCategory::Security,
    description: "Oracle price used without freshness validation (CFG-aware dataflow, requires --mode full --preview)",
    group: RuleGroup::Preview,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBasedCFG,
    gap: Some(TypeSystemGap::TemporalOrdering),
};

// ============================================================================
// 1. Unused Capability Parameter (CFG-aware with Rich Value Tracking)
// ============================================================================

pub struct UnusedCapabilityVerifier;

pub struct UnusedCapabilityVerifierAI<'a> {
    /// Capability parameters to track
    cap_params: Vec<(Var, Loc)>,
    /// Module typing information
    info: &'a TypingProgramInfo,
    /// CFG for block analysis (abort detection)
    cfg: Option<&'a ImmForwardCFG<'a>>,
    /// Whether the function performed a privileged sink operation.
    has_privileged_sink: Cell<bool>,
    /// Tracks whether each capability was referenced anywhere during analysis.
    cap_used_anywhere: RefCell<BTreeMap<String, bool>>,
    /// Tracks whether each capability was validated (survives Move)
    cap_validated_anywhere: RefCell<BTreeMap<String, bool>>,
}

/// Abstract value for capability tracking with rich validation states.
/// Forms a lattice: Unused < AccessedNotValidated < PendingValidation < Validated
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CapValue {
    /// Capability parameter never accessed
    Unused,
    /// Capability was accessed (field read, passed to function) but result
    /// was not used in a guard condition
    AccessedNotValidated(Loc),
    /// A validation check was computed (cap.id == x) but hasn't flowed
    /// into a guard (assert/if) yet
    PendingValidation(Loc),
    /// Capability was validated through a guard (assert!/if condition)
    Validated(Loc),
}

impl Default for CapValue {
    fn default() -> Self {
        CapValue::Unused
    }
}

impl PartialOrd for CapValue {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CapValue {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        fn rank(v: &CapValue) -> u8 {
            match v {
                CapValue::Unused => 0,
                CapValue::AccessedNotValidated(_) => 1,
                CapValue::PendingValidation(_) => 2,
                CapValue::Validated(_) => 3,
            }
        }
        rank(self).cmp(&rank(other))
    }
}

/// Execution context for capability tracking
pub struct CapExecutionContext {
    diags: CompilerDiagnostics,
}

/// Abstract state: mapping from variables to capability usage
#[derive(Clone, Debug)]
pub struct CapState {
    locals: BTreeMap<Var, LocalState<CapValue>>,
    /// Track whether each capability has been used (survives Move)
    cap_used: BTreeMap<String, bool>,
    /// Track whether each capability has been validated (survives Move)
    cap_validated: BTreeMap<String, bool>,
}

impl SimpleAbsIntConstructor for UnusedCapabilityVerifier {
    type AI<'a> = UnusedCapabilityVerifierAI<'a>;

    fn new<'a>(
        context: &'a CFGContext<'a>,
        cfg: &ImmForwardCFG,
        init_state: &mut CapState,
    ) -> Option<Self::AI<'a>> {
        // Skip test functions
        if context.attributes.is_test_or_test_only() {
            return None;
        }

        // Find capability parameters (key+store, no copy/drop, reference type)
        let cap_params: Vec<(Var, Loc)> = context
            .signature
            .parameters
            .iter()
            .filter_map(|(_, var, ty)| {
                if is_auth_token_param(var, ty) {
                    Some((*var, var.0.loc))
                } else {
                    None
                }
            })
            .collect();

        if cap_params.is_empty() {
            return None; // No analysis needed
        }

        // Initialize all capability parameters as Unused
        let mut cap_used_anywhere = BTreeMap::new();
        let mut cap_validated_anywhere = BTreeMap::new();
        for (cap_var, _loc) in &cap_params {
            if let Some(LocalState::Available(_, value)) = init_state.locals.get_mut(cap_var) {
                *value = CapValue::Unused;
            }

            let sym = cap_var.value();
            let key = sym.as_str().to_owned();
            init_state.cap_used.insert(key.clone(), false);
            init_state.cap_validated.insert(key.clone(), false);
            cap_used_anywhere.insert(key.clone(), false);
            cap_validated_anywhere.insert(key, false);
        }

        // NOTE: We can't store the cfg reference directly due to lifetime constraints
        // in SimpleAbsIntConstructor. The cfg parameter has a different lifetime than 'a.
        // We work around this by not storing cfg and using conservative abort detection.
        Some(UnusedCapabilityVerifierAI {
            cap_params,
            info: context.info,
            cfg: None, // Can't store due to lifetime mismatch
            has_privileged_sink: Cell::new(false),
            cap_used_anywhere: RefCell::new(cap_used_anywhere),
            cap_validated_anywhere: RefCell::new(cap_validated_anywhere),
        })
    }
}

impl SimpleAbsInt for UnusedCapabilityVerifierAI<'_> {
    type State = CapState;
    type ExecutionContext = CapExecutionContext;

    fn finish(
        &mut self,
        final_states: BTreeMap<Label, Self::State>,
        diags: CompilerDiagnostics,
    ) -> CompilerDiagnostics {
        let mut result_diags = diags;

        // Only run this check when the function performs a privileged sink.
        if !self.has_privileged_sink.get() {
            return result_diags;
        }

        let used_anywhere = self.cap_used_anywhere.borrow();
        let validated_anywhere = self.cap_validated_anywhere.borrow();

        for (cap_var, cap_loc) in &self.cap_params {
            let cap_sym = cap_var.value();
            let cap_key = cap_sym.as_str();
            let was_used = used_anywhere.get(cap_key).copied().unwrap_or(false);
            let was_validated = validated_anywhere.get(cap_key).copied().unwrap_or(false);

            if !was_used {
                // Completely unused - strongest warning
                let msg = format!(
                    "Capability parameter `{}` is never accessed, indicating missing access control",
                    cap_key
                );
                let help =
                    "Add validation: `assert!(cap.pool_id == object::id(pool), E_WRONG_CAP)`";
                result_diags.add(diag!(UNUSED_CAP_V2_DIAG, (*cap_loc, msg), (*cap_loc, help)));
                continue;
            }

            // Check final validation state across all exit points
            if !was_validated {
                // Check if it was accessed but not validated in any final state
                let any_unvalidated = final_states.values().any(|state| {
                    match state.locals.get(cap_var) {
                        Some(LocalState::Available(_, CapValue::Unused))
                        | Some(LocalState::Available(_, CapValue::AccessedNotValidated(_)))
                        | Some(LocalState::Available(_, CapValue::PendingValidation(_))) => true,
                        Some(LocalState::Unavailable(_, _)) => {
                            // Moved - check cap_validated
                            !state.cap_validated.get(cap_key).copied().unwrap_or(false)
                        }
                        Some(LocalState::Available(_, CapValue::Validated(_))) => false,
                        _ => true,
                    }
                });

                if any_unvalidated {
                    let msg = format!(
                        "Capability parameter `{}` is accessed but not validated through a guard",
                        cap_key
                    );
                    let help =
                        "Capability field accesses should flow into `assert!` or `if` conditions";
                    result_diags.add(diag!(
                        UNVALIDATED_CAP_V2_DIAG,
                        (*cap_loc, msg),
                        (*cap_loc, help)
                    ));
                }
            }
        }

        result_diags
    }

    fn start_command(&self, _pre: &mut Self::State) -> Self::ExecutionContext {
        CapExecutionContext {
            diags: CompilerDiagnostics::new(),
        }
    }

    fn finish_command(
        &self,
        context: Self::ExecutionContext,
        _state: &mut Self::State,
    ) -> CompilerDiagnostics {
        context.diags
    }

    fn command_custom(
        &self,
        context: &mut Self::ExecutionContext,
        state: &mut Self::State,
        cmd: &Command,
    ) -> bool {
        match &cmd.value {
            // JumpIf handles both `if` and `assert!` (compiled to JumpIf + abort branch)
            Command_::JumpIf {
                cond,
                if_true,
                if_false,
            } => {
                // Visit condition to update state
                self.exp(context, state, cond);

                // For now, we use a conservative heuristic: any JumpIf with a capability
                // in the condition is treated as a potential validation.
                // A more precise implementation would check if one branch aborts.
                // Since we can't store the CFG reference, we assume all JumpIf conditions
                // with capability accesses are guards (conservative for false negatives).
                self.mark_validated_in_condition(state, cond);

                true // Handled
            }
            _ => false,
        }
    }

    fn exp_custom(
        &self,
        context: &mut Self::ExecutionContext,
        state: &mut Self::State,
        e: &Exp,
    ) -> Option<Vec<CapValue>> {
        use UnannotatedExp_ as E;

        match &e.exp.value {
            // Direct capability access (borrow, move, copy)
            E::BorrowLocal(_, var) | E::Move { var, .. } | E::Copy { var, .. } => {
                if self.is_tracked_cap(var) {
                    let sym = var.value();
                    let key = sym.as_str().to_owned();

                    // Mark accessed
                    state.cap_used.insert(key.clone(), true);
                    self.cap_used_anywhere
                        .borrow_mut()
                        .insert(key.clone(), true);

                    // Update local state - upgrade from Unused to AccessedNotValidated
                    if let Some(local_state) = state.locals.get_mut(var)
                        && let LocalState::Available(loc, val) = local_state
                        && *val == CapValue::Unused
                    {
                        *local_state =
                            LocalState::Available(*loc, CapValue::AccessedNotValidated(e.exp.loc));
                    }

                    // Return current state for propagation
                    return Some(vec![self.get_cap_value(state, var)]);
                }
            }

            // Field access on capability: cap.field
            E::Borrow(_, inner, _field, _) => {
                if let E::BorrowLocal(_, var) | E::Copy { var, .. } | E::Move { var, .. } =
                    &inner.exp.value
                    && self.is_tracked_cap(var)
                {
                    // Field access is an "access" but not validation
                    self.mark_accessed(state, var, e.exp.loc);
                    // Continue traversing inner
                    self.exp(context, state, inner);
                    return Some(vec![CapValue::AccessedNotValidated(e.exp.loc)]);
                }
            }

            // Comparison involving capability: cap.id == x
            E::BinopExp(lhs, op, rhs) => {
                // Check if this is a validation comparison
                if matches!(
                    op.value,
                    BinOp_::Eq | BinOp_::Neq | BinOp_::Lt | BinOp_::Gt | BinOp_::Le | BinOp_::Ge
                ) {
                    let lhs_vals = self.exp(context, state, lhs);
                    let rhs_vals = self.exp(context, state, rhs);

                    // If either side involves a capability access, this is a PendingValidation
                    let lhs_is_cap = lhs_vals.iter().any(|v| {
                        matches!(
                            v,
                            CapValue::AccessedNotValidated(_) | CapValue::PendingValidation(_)
                        )
                    });
                    let rhs_is_cap = rhs_vals.iter().any(|v| {
                        matches!(
                            v,
                            CapValue::AccessedNotValidated(_) | CapValue::PendingValidation(_)
                        )
                    });

                    if lhs_is_cap || rhs_is_cap {
                        // This comparison produces a PendingValidation bool
                        return Some(vec![CapValue::PendingValidation(e.exp.loc)]);
                    }
                }
            }

            _ => {}
        }

        None // Use default traversal
    }

    fn call_custom(
        &self,
        _context: &mut Self::ExecutionContext,
        state: &mut Self::State,
        _loc: &Loc,
        _return_ty: &Type,
        f: &ModuleCall,
        _args: Vec<CapValue>,
    ) -> Option<Vec<CapValue>> {
        // Type-based privileged sink detection
        if self.is_privileged_sink_call(f) {
            self.has_privileged_sink.set(true);
        }

        None
    }
}

impl UnusedCapabilityVerifierAI<'_> {
    fn is_tracked_cap(&self, var: &Var) -> bool {
        let sym = var.value();
        self.cap_params.iter().any(|(cap, _)| cap.value() == sym)
    }

    fn get_cap_value(&self, state: &CapState, var: &Var) -> CapValue {
        match state.locals.get(var) {
            Some(LocalState::Available(_, val)) => *val,
            _ => CapValue::Unused,
        }
    }

    fn mark_accessed(&self, state: &mut CapState, var: &Var, loc: Loc) {
        let key = var.value().as_str().to_owned();
        state.cap_used.insert(key.clone(), true);
        self.cap_used_anywhere.borrow_mut().insert(key, true);

        if let Some(local_state) = state.locals.get_mut(var)
            && let LocalState::Available(l, val) = local_state
            && *val == CapValue::Unused
        {
            *local_state = LocalState::Available(*l, CapValue::AccessedNotValidated(loc));
        }
    }

    fn mark_validated_in_condition(&self, state: &mut CapState, cond: &Exp) {
        // Find all capability variables referenced in condition
        let cap_accesses = self.extract_capability_accesses(cond);

        for (cap_var, loc) in cap_accesses {
            if self.is_tracked_cap(&cap_var) {
                let key = cap_var.value().as_str().to_owned();

                // Mark as validated
                state.cap_validated.insert(key.clone(), true);
                self.cap_validated_anywhere
                    .borrow_mut()
                    .insert(key.clone(), true);

                // Upgrade to Validated state
                if let Some(local_state) = state.locals.get_mut(&cap_var) {
                    match local_state {
                        LocalState::Available(l, CapValue::Unused)
                        | LocalState::Available(l, CapValue::AccessedNotValidated(_))
                        | LocalState::Available(l, CapValue::PendingValidation(_)) => {
                            *local_state = LocalState::Available(*l, CapValue::Validated(loc));
                        }
                        LocalState::Available(_, CapValue::Validated(_)) => {
                            // Already validated, keep it
                        }
                        _ => {}
                    }
                }

                state.cap_used.insert(key.clone(), true);
                self.cap_used_anywhere.borrow_mut().insert(key, true);
            }
        }
    }

    fn extract_capability_accesses(&self, exp: &Exp) -> Vec<(Var, Loc)> {
        let mut accesses = Vec::new();
        self.collect_cap_accesses_recursive(exp, &mut accesses);
        accesses
    }

    fn collect_cap_accesses_recursive(&self, exp: &Exp, accesses: &mut Vec<(Var, Loc)>) {
        use UnannotatedExp_ as E;
        match &exp.exp.value {
            // Field access: cap.field
            E::Borrow(_, inner, _field, _) => {
                if let E::BorrowLocal(_, var) | E::Copy { var, .. } | E::Move { var, .. } =
                    &inner.exp.value
                    && self.is_tracked_cap(var)
                {
                    accesses.push((*var, exp.exp.loc));
                }
                self.collect_cap_accesses_recursive(inner, accesses);
            }
            // Direct variable access
            E::BorrowLocal(_, var) | E::Copy { var, .. } | E::Move { var, .. } => {
                if self.is_tracked_cap(var) {
                    accesses.push((*var, exp.exp.loc));
                }
            }
            // Binary comparisons (cap.id == pool.id)
            E::BinopExp(lhs, _, rhs) => {
                self.collect_cap_accesses_recursive(lhs, accesses);
                self.collect_cap_accesses_recursive(rhs, accesses);
            }
            // Unary ops
            E::UnaryExp(_, inner)
            | E::Dereference(inner)
            | E::Freeze(inner)
            | E::Cast(inner, _) => {
                self.collect_cap_accesses_recursive(inner, accesses);
            }
            // Function calls in conditions (validation functions)
            E::ModuleCall(call) => {
                for arg in &call.arguments {
                    self.collect_cap_accesses_recursive(arg, accesses);
                }
            }
            // Vector elements
            E::Vector(_, _, _, args) => {
                for arg in args {
                    self.collect_cap_accesses_recursive(arg, accesses);
                }
            }
            // Multiple expressions
            E::Multiple(es) => {
                for e in es {
                    self.collect_cap_accesses_recursive(e, accesses);
                }
            }
            // Pack expressions
            E::Pack(_, _, fields) | E::PackVariant(_, _, _, fields) => {
                for (_, _, e) in fields {
                    self.collect_cap_accesses_recursive(e, accesses);
                }
            }
            _ => {}
        }
    }

    /// Type-based privileged sink detection.
    /// A privileged sink is any function that:
    /// 1. Is a known transfer/state-mutation function
    /// 2. Takes &mut to a value-bearing resource (key+store with value fields)
    /// 3. Returns a value-bearing resource (extraction)
    fn is_privileged_sink_call(&self, call: &ModuleCall) -> bool {
        // Check known transfer functions (definitive sinks)
        if self.is_known_transfer_function(call) {
            return true;
        }

        // Check if any argument is &mut Resource where Resource has value
        for arg in &call.arguments {
            if self.is_mutable_value_resource_ref(&arg.ty) {
                return true;
            }
        }

        false
    }

    fn is_known_transfer_function(&self, call: &ModuleCall) -> bool {
        call.is(&SUI_ADDR, "transfer", "transfer")
            || call.is(&SUI_ADDR, "transfer", "public_transfer")
            || call.is(&SUI_ADDR, "transfer", "share_object")
            || call.is(&SUI_ADDR, "transfer", "public_share_object")
            || call.is(&SUI_ADDR, "transfer", "freeze_object")
            || call.is(&SUI_ADDR, "balance", "increase_supply")
            || call.is(&SUI_ADDR, "balance", "decrease_supply")
            || call.is(&SUI_ADDR, "balance", "split")
            || call.is(&SUI_ADDR, "balance", "withdraw_all")
            || call.is(&SUI_ADDR, "coin", "take")
            || call.is(&SUI_ADDR, "coin", "split")
            || call.is(&SUI_ADDR, "coin", "mint")
            || call.is(&SUI_ADDR, "coin", "burn")
    }

    fn is_mutable_value_resource_ref(&self, ty: &Type) -> bool {
        if let Type_::Single(st) = &ty.value
            && let SingleType_::Ref(true, bt) = &st.value
        {
            return self.is_value_bearing_resource(bt);
        }
        false
    }

    fn is_value_bearing_resource(&self, bt: &BaseType) -> bool {
        match &bt.value {
            BaseType_::Apply(abilities, type_name, _) => {
                // Check if it's a ModuleType
                if let TypeName_::ModuleType(m, n) = &type_name.value {
                    // Must have key+store (object that can be transferred)
                    if !abilities.has_ability_(Ability_::Key)
                        || !abilities.has_ability_(Ability_::Store)
                    {
                        return false;
                    }
                    // Check if struct has numeric/balance fields (heuristic for "value")
                    if let Some(sdef) = self.info.struct_definition_opt(m, n) {
                        return self.struct_has_value_fields_hlir(sdef);
                    }
                    // If we can't find the struct, be conservative and treat it as value-bearing
                    return true;
                }
                false
            }
            _ => false,
        }
    }

    fn struct_has_value_fields_hlir(
        &self,
        sdef: &move_compiler::naming::ast::StructDefinition,
    ) -> bool {
        match &sdef.fields {
            NStructFields::Defined(_, fields) => fields
                .iter()
                .any(|(_, _, (_, (_, ty)))| self.is_numeric_or_balance_type(ty)),
            NStructFields::Native(_) => false,
        }
    }

    fn is_numeric_or_balance_type(&self, ty: &NType) -> bool {
        match &ty.value {
            NType_::Apply(_, type_name, _) => {
                match &type_name.value {
                    move_compiler::naming::ast::TypeName_::ModuleType(m, n) => {
                        // Check for Balance<T> type
                        m.value.is(&SUI_ADDR, "balance") && n.0.value.as_str() == "Balance"
                    }
                    move_compiler::naming::ast::TypeName_::Builtin(builtin) => {
                        matches!(
                            builtin.value,
                            BuiltinTypeName_::U8
                                | BuiltinTypeName_::U16
                                | BuiltinTypeName_::U32
                                | BuiltinTypeName_::U64
                                | BuiltinTypeName_::U128
                                | BuiltinTypeName_::U256
                        )
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }
}

impl SimpleDomain for CapState {
    type Value = CapValue;

    fn new(_context: &CFGContext, locals: BTreeMap<Var, LocalState<Self::Value>>) -> Self {
        CapState {
            locals,
            cap_used: BTreeMap::new(),
            cap_validated: BTreeMap::new(),
        }
    }

    fn locals_mut(&mut self) -> &mut BTreeMap<Var, LocalState<Self::Value>> {
        &mut self.locals
    }

    fn locals(&self) -> &BTreeMap<Var, LocalState<Self::Value>> {
        &self.locals
    }

    fn join_value(v1: &Self::Value, v2: &Self::Value) -> Self::Value {
        use CapValue::*;
        match (v1, v2) {
            // Once validated on any path, consider validated (optimistic)
            (Validated(loc), _) | (_, Validated(loc)) => Validated(*loc),
            // Pending validation if either path has it
            (PendingValidation(loc), _) | (_, PendingValidation(loc)) => PendingValidation(*loc),
            // Accessed but not validated
            (AccessedNotValidated(loc), _) | (_, AccessedNotValidated(loc)) => {
                AccessedNotValidated(*loc)
            }
            // Both unused
            (Unused, Unused) => Unused,
        }
    }

    fn join_impl(&mut self, other: &Self, _result: &mut JoinResult) {
        for (var, used) in &other.cap_used {
            let entry = self.cap_used.entry(var.clone()).or_insert(false);
            *entry = *entry || *used;
        }
        for (var, validated) in &other.cap_validated {
            let entry = self.cap_validated.entry(var.clone()).or_insert(false);
            *entry = *entry || *validated;
        }
    }
}

impl SimpleExecutionContext for CapExecutionContext {
    fn add_diag(&mut self, diag: CompilerDiagnostic) {
        self.diags.add(diag)
    }
}

/// Check if a parameter is an auth-token candidate.
///
/// Policy (preview, type-based):
/// - Reference parameter (`&T` / `&mut T`).
/// - `key + store` resource-like type.
/// - Excludes `copy` and `drop`.
/// - Underscore-prefixed parameters are treated as intentional auth-by-presence.
fn is_auth_token_param(var: &Var, ty: &SingleType) -> bool {
    if var.starts_with_underscore() {
        return false;
    }

    let bt = match &ty.value {
        SingleType_::Ref(_, bt) => bt,
        _ => return false,
    };

    is_auth_token_base_type(&bt.value)
}

fn is_auth_token_base_type(bt: &BaseType_) -> bool {
    matches!(bt, BaseType_::Apply(abilities, _type_name, _) if abilities.has_ability_(Ability_::Key)
                && abilities.has_ability_(Ability_::Store)
                && !abilities.has_ability_(Ability_::Copy)
                && !abilities.has_ability_(Ability_::Drop))
}

// ============================================================================
// 2. Unchecked Division (CFG-aware)
// ============================================================================

pub struct UncheckedDivisionVerifier;

pub struct UncheckedDivisionVerifierAI<'a> {
    info: &'a TypingProgramInfo,
    context: &'a CFGContext<'a>,
    /// Labels whose blocks immediately exit (abort/return), used to infer guards from `if`.
    exit_blocks: BTreeSet<Label>,
}

/// Abstract value: tracks whether a divisor has been validated
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum DivisorValue {
    /// Unknown validation status
    #[default]
    Unknown,
    /// Proven non-zero via assertion
    Validated,
    /// Known to be a constant (safe)
    Constant,
}

pub struct DivExecutionContext {
    diags: CompilerDiagnostics,
}

#[derive(Clone, Debug)]
pub struct DivState {
    locals: BTreeMap<Var, LocalState<DivisorValue>>,
}

impl SimpleAbsIntConstructor for UncheckedDivisionVerifier {
    type AI<'a> = UncheckedDivisionVerifierAI<'a>;

    fn new<'a>(
        context: &'a CFGContext<'a>,
        cfg: &ImmForwardCFG,
        _init_state: &mut DivState,
    ) -> Option<Self::AI<'a>> {
        if context.attributes.is_test_or_test_only() {
            return None;
        }

        // We can't store `cfg` directly due to lifetime constraints, but we can precompute
        // per-block metadata we need for guard inference.
        let mut exit_blocks = BTreeSet::new();
        for lbl in cfg.block_labels() {
            if is_immediate_exit_block(cfg, lbl) {
                exit_blocks.insert(lbl);
            }
        }
        Some(UncheckedDivisionVerifierAI {
            info: context.info,
            context,
            exit_blocks,
        })
    }
}

impl SimpleAbsInt for UncheckedDivisionVerifierAI<'_> {
    type State = DivState;
    type ExecutionContext = DivExecutionContext;

    fn finish(
        &mut self,
        _final_states: BTreeMap<Label, Self::State>,
        diags: CompilerDiagnostics,
    ) -> CompilerDiagnostics {
        diags
    }

    fn start_command(&self, _pre: &mut Self::State) -> Self::ExecutionContext {
        DivExecutionContext {
            diags: CompilerDiagnostics::new(),
        }
    }

    fn finish_command(
        &self,
        context: Self::ExecutionContext,
        _state: &mut Self::State,
    ) -> CompilerDiagnostics {
        context.diags
    }

    fn command_custom(
        &self,
        context: &mut Self::ExecutionContext,
        state: &mut Self::State,
        cmd: &Command,
    ) -> bool {
        use Command_ as C;

        match &cmd.value {
            C::JumpIf {
                cond,
                if_true,
                if_false,
            } => {
                self.exp(context, state, cond);

                let true_is_abort = self.block_is_immediate_abort(*if_true);
                let false_is_abort = self.block_is_immediate_abort(*if_false);

                if true_is_abort ^ false_is_abort {
                    let cond_true_on_continue = false_is_abort;
                    self.track_nonzero_guard(state, cond, cond_true_on_continue);
                }

                true
            }
            _ => false,
        }
    }

    fn exp_custom(
        &self,
        context: &mut Self::ExecutionContext,
        state: &mut Self::State,
        e: &Exp,
    ) -> Option<Vec<DivisorValue>> {
        use UnannotatedExp_ as E;

        match &e.exp.value {
            E::Value(v) => {
                let is_zero = v.value.is_zero();
                return Some(vec![if is_zero {
                    DivisorValue::Unknown
                } else {
                    DivisorValue::Constant
                }]);
            }
            E::Constant(_) => return Some(vec![DivisorValue::Constant]),
            E::Cast(inner, _ty) => {
                let mut vs = self.exp(context, state, inner);
                vs.truncate(1);
                if vs.is_empty() {
                    vs.push(DivisorValue::Unknown);
                }
                return Some(vs);
            }
            E::BinopExp(lhs, op, rhs) => match op.value {
                BinOp_::Div | BinOp_::Mod => {
                    self.exp(context, state, lhs);
                    let divisor_safe = self.is_divisor_safe(state, rhs);
                    self.exp(context, state, rhs);

                    if !divisor_safe {
                        if !self.is_root_source_loc(self.context, &e.exp.loc) {
                            return Some(vec![DivisorValue::Unknown]);
                        }
                        self.debug_provenance_if_enabled(self.context, &e.exp.loc);
                        let msg = "Division or modulo operation without zero-check validation";
                        let help = "Add validation: `assert!(divisor != 0, E_DIVISION_BY_ZERO)`";
                        let d =
                            diag!(UNCHECKED_DIV_V2_DIAG, (e.exp.loc, msg), (rhs.exp.loc, help),);
                        context.add_diag(d);
                    }
                    return Some(vec![DivisorValue::Unknown]);
                }
                BinOp_::Add => {
                    let lvals = self.exp(context, state, lhs);
                    let rvals = self.exp(context, state, rhs);
                    let l = lvals.first().copied().unwrap_or_default();
                    let r = rvals.first().copied().unwrap_or_default();

                    let rhs_is_nonzero_lit =
                        matches!(&rhs.exp.value, E::Value(v) if !v.value.is_zero());
                    let lhs_is_nonzero_lit =
                        matches!(&lhs.exp.value, E::Value(v) if !v.value.is_zero());

                    let inferred = if rhs_is_nonzero_lit || lhs_is_nonzero_lit {
                        DivisorValue::Validated
                    } else if l == DivisorValue::Constant && r == DivisorValue::Constant {
                        DivisorValue::Constant
                    } else {
                        DivisorValue::Unknown
                    };

                    return Some(vec![inferred]);
                }
                _ => {}
            },
            E::ModuleCall(call) => {
                if self.is_assert_call(call)
                    && let Some(cond) = call.arguments.first()
                {
                    self.track_nonzero_guard(state, cond, true);
                }
            }
            _ => {}
        }

        None
    }
}

impl UncheckedDivisionVerifierAI<'_> {
    fn block_is_immediate_abort(&self, lbl: Label) -> bool {
        self.exit_blocks.contains(&lbl)
    }

    fn is_root_source_loc(&self, context: &CFGContext, _loc: &Loc) -> bool {
        // Check if this is root package code (not a dependency)
        let is_dependency = context.env.package_config(context.package).is_dependency;
        !is_dependency
    }

    fn debug_provenance_if_enabled(&self, context: &CFGContext, loc: &Loc) {
        if std::env::var("MOVE_CLIPPY_DEBUG_PHASE2_PROVENANCE").as_deref() != Ok("1") {
            return;
        }

        let file_hash = loc.file_hash();
        let file_path = context
            .env
            .mapped_files()
            .file_path(&file_hash)
            .to_string_lossy();
        let is_dependency = context.env.package_config(context.package).is_dependency;

        eprintln!(
            "[move-clippy][phase2][unchecked_division_v2] file={} package={:?} is_dependency={}",
            file_path, context.package, is_dependency,
        );
    }

    #[allow(clippy::only_used_in_recursion)]
    fn is_divisor_safe(&self, state: &DivState, divisor: &Exp) -> bool {
        match &divisor.exp.value {
            UnannotatedExp_::Value(v) => !v.value.is_zero(),
            UnannotatedExp_::Constant(_) => true,
            UnannotatedExp_::Move { var, .. }
            | UnannotatedExp_::Copy { var, .. }
            | UnannotatedExp_::BorrowLocal(_, var) => matches!(
                state.locals.get(var),
                Some(LocalState::Available(
                    _,
                    DivisorValue::Validated | DivisorValue::Constant
                ))
            ),
            UnannotatedExp_::Cast(inner, _ty) => self.is_divisor_safe(state, inner),
            UnannotatedExp_::BinopExp(lhs, op, rhs)
                if matches!(op.value, BinOp_::Add)
                    && (matches!(&lhs.exp.value, UnannotatedExp_::Value(v) if !v.value.is_zero())
                        || matches!(&rhs.exp.value, UnannotatedExp_::Value(v) if !v.value.is_zero())) =>
            {
                true
            }
            _ => true,
        }
    }

    fn is_assert_call(&self, call: &ModuleCall) -> bool {
        let func_sym = call.name.value();
        let func_name = func_sym.as_str();
        func_name == "assert" || func_name.contains("assert")
    }

    fn track_nonzero_guard(&self, state: &mut DivState, condition: &Exp, cond_value: bool) {
        if let Some((var, nonzero_when_true)) = self.extract_nonzero_guard(state, condition) {
            let implies_nonzero = if cond_value {
                nonzero_when_true
            } else {
                !nonzero_when_true
            };

            if implies_nonzero {
                self.mark_nonzero(state, var);
            }
        }
    }

    fn extract_nonzero_guard(&self, state: &DivState, condition: &Exp) -> Option<(Var, bool)> {
        match &condition.exp.value {
            UnannotatedExp_::UnaryExp(unop, inner)
                if matches!(unop.value, move_compiler::parser::ast::UnaryOp_::Not) =>
            {
                self.extract_nonzero_guard(state, inner)
                    .map(|(v, nonzero_when_true)| (v, !nonzero_when_true))
            }
            UnannotatedExp_::Cast(inner, _) => self.extract_nonzero_guard(state, inner),
            UnannotatedExp_::BinopExp(lhs, op, rhs) => {
                // Handle `a && b` by extracting from either side (best-effort).
                if matches!(op.value, BinOp_::And) {
                    return self
                        .extract_nonzero_guard(state, lhs)
                        .or_else(|| self.extract_nonzero_guard(state, rhs));
                }

                // Helper to check if an expression is a constant-like value.
                // This includes:
                // - Direct Value literals
                // - Direct Constant references
                // - Copy/Move of a local that is known to hold a constant value
                let is_const_like_with_state = |e: &Exp| -> bool {
                    match &e.exp.value {
                        UnannotatedExp_::Value(_) | UnannotatedExp_::Constant(_) => true,
                        UnannotatedExp_::Copy { var, .. } | UnannotatedExp_::Move { var, .. } => {
                            // Check if this local is known to hold a constant (non-zero) value
                            matches!(
                                state.locals.get(var),
                                Some(LocalState::Available(_, DivisorValue::Constant))
                            )
                        }
                        _ => false,
                    }
                };

                let (var_side, lit_side, var_on_lhs) = if is_const_like_with_state(rhs) {
                    (lhs, rhs, true)
                } else if is_const_like_with_state(lhs) {
                    (rhs, lhs, false)
                } else {
                    return None;
                };

                let var = self.extract_var(var_side)?;

                // Determine if the constant/literal is zero
                let lit_is_zero = match &lit_side.exp.value {
                    UnannotatedExp_::Value(v) => v.value.is_zero(),
                    UnannotatedExp_::Constant(_) => {
                        // Named constants are assumed non-zero (conservative but practical)
                        false
                    }
                    UnannotatedExp_::Copy { var, .. } | UnannotatedExp_::Move { var, .. } => {
                        // Local holds a constant value - we tracked it, so it's non-zero
                        if matches!(
                            state.locals.get(var),
                            Some(LocalState::Available(_, DivisorValue::Constant))
                        ) {
                            false // It's a non-zero constant
                        } else {
                            return None; // Unknown value
                        }
                    }
                    _ => return None,
                };

                // Returns whether the condition being TRUE implies the var is non-zero.
                let nonzero_when_true = match (op.value, lit_is_zero, var_on_lhs) {
                    // var != 0  OR  0 != var
                    (BinOp_::Neq, true, _) => true,
                    (BinOp_::Eq, true, _) => false,

                    // var > 0
                    (BinOp_::Gt, true, true) => true,
                    // 0 < var
                    (BinOp_::Lt, true, false) => true,

                    // var >= K where K != 0 (includes named constants)
                    (BinOp_::Ge, false, true) => true,
                    // K <= var where K != 0 (includes named constants)
                    (BinOp_::Le, false, false) => true,

                    // var == K where K != 0
                    (BinOp_::Eq, false, _) => true,

                    _ => return None,
                };

                Some((var, nonzero_when_true))
            }
            _ => None,
        }
    }

    fn extract_var(&self, exp: &Exp) -> Option<Var> {
        match &exp.exp.value {
            UnannotatedExp_::Move { var, .. } | UnannotatedExp_::Copy { var, .. } => Some(*var),
            UnannotatedExp_::BorrowLocal(_, var) => Some(*var),
            _ => None,
        }
    }

    fn mark_nonzero(&self, state: &mut DivState, var: Var) {
        if let Some(local_state) = state.locals.get_mut(&var) {
            match local_state {
                LocalState::Available(loc, _) => {
                    *local_state = LocalState::Available(*loc, DivisorValue::Validated);
                }
                LocalState::MaybeUnavailable { available, .. } => {
                    let loc = *available;
                    *local_state = LocalState::Available(loc, DivisorValue::Validated);
                }
                _ => {}
            }
        }
    }
}

fn is_immediate_exit_block(cfg: &ImmForwardCFG, lbl: Label) -> bool {
    use move_compiler::hlir::ast::Command_ as C;

    let cmds: Vec<_> = cfg.commands(lbl).map(|(_i, c)| &c.value).collect();
    if cmds.is_empty() {
        return false;
    }

    // Allow a small amount of cleanup (IgnoreAndPop) before the terminal exit.
    let (prefix, last) = cmds.split_at(cmds.len().saturating_sub(1));
    let is_exit = matches!(last.first(), Some(C::Abort(_, _) | C::Return { .. }));
    if !is_exit {
        return false;
    }
    prefix.iter().all(|c| matches!(c, C::IgnoreAndPop { .. }))
}

impl SimpleDomain for DivState {
    type Value = DivisorValue;

    fn new(_context: &CFGContext, locals: BTreeMap<Var, LocalState<Self::Value>>) -> Self {
        DivState { locals }
    }

    fn locals_mut(&mut self) -> &mut BTreeMap<Var, LocalState<Self::Value>> {
        &mut self.locals
    }

    fn locals(&self) -> &BTreeMap<Var, LocalState<Self::Value>> {
        &self.locals
    }

    fn join_value(v1: &Self::Value, v2: &Self::Value) -> Self::Value {
        use DivisorValue::*;
        match (v1, v2) {
            (Constant, Constant) => Constant,
            (Validated, Validated) => Validated,
            (Constant, Validated) | (Validated, Constant) => Validated,
            _ => Unknown,
        }
    }

    fn join_impl(&mut self, _other: &Self, _result: &mut JoinResult) {}
}

impl SimpleExecutionContext for DivExecutionContext {
    fn add_diag(&mut self, d: CompilerDiagnostic) {
        self.diags.add(d);
    }
}

// ============================================================================
// 3. Destroy Zero Unchecked V2 (CFG-aware)
// ============================================================================

const DESTROY_ZERO_UNCHECKED_V2_DIAG: DiagnosticInfo = custom(
    LINT_WARNING_PREFIX,
    Severity::Warning,
    CLIPPY_CATEGORY,
    4, // destroy_zero_unchecked_v2
    "destroy_zero called without prior zero-check",
);

pub static DESTROY_ZERO_UNCHECKED_V2: LintDescriptor = LintDescriptor {
    name: "destroy_zero_unchecked_v2",
    category: LintCategory::Security,
    description: "destroy_zero called without verifying value is zero (CFG-aware, requires --mode full --preview)",
    group: RuleGroup::Preview,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBasedCFG,
    gap: Some(TypeSystemGap::ValueFlow),
};

pub struct DestroyZeroVerifier;

pub struct DestroyZeroVerifierAI<'a> {
    info: &'a TypingProgramInfo,
    context: &'a CFGContext<'a>,
    exit_blocks: BTreeSet<Label>,
}

/// Abstract value for tracking zero-check status
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum ZeroCheckValue {
    /// Unknown - no zero check seen
    #[default]
    Unknown,
    /// Checked to be zero via assert/if
    CheckedZero,
    /// Known constant zero
    ConstantZero,
}

pub struct DestroyZeroExecutionContext {
    diags: CompilerDiagnostics,
}

#[derive(Clone, Debug)]
pub struct DestroyZeroState {
    locals: BTreeMap<Var, LocalState<ZeroCheckValue>>,
}

impl SimpleAbsIntConstructor for DestroyZeroVerifier {
    type AI<'a> = DestroyZeroVerifierAI<'a>;

    fn new<'a>(
        context: &'a CFGContext<'a>,
        cfg: &ImmForwardCFG,
        _init_state: &mut DestroyZeroState,
    ) -> Option<Self::AI<'a>> {
        if context.attributes.is_test_or_test_only() {
            return None;
        }

        let mut exit_blocks = BTreeSet::new();
        for lbl in cfg.block_labels() {
            if is_immediate_exit_block(cfg, lbl) {
                exit_blocks.insert(lbl);
            }
        }

        Some(DestroyZeroVerifierAI {
            info: context.info,
            context,
            exit_blocks,
        })
    }
}

impl SimpleAbsInt for DestroyZeroVerifierAI<'_> {
    type State = DestroyZeroState;
    type ExecutionContext = DestroyZeroExecutionContext;

    fn finish(
        &mut self,
        _final_states: BTreeMap<Label, Self::State>,
        diags: CompilerDiagnostics,
    ) -> CompilerDiagnostics {
        diags
    }

    fn start_command(&self, _pre: &mut Self::State) -> Self::ExecutionContext {
        DestroyZeroExecutionContext {
            diags: CompilerDiagnostics::new(),
        }
    }

    fn finish_command(
        &self,
        context: Self::ExecutionContext,
        _state: &mut Self::State,
    ) -> CompilerDiagnostics {
        context.diags
    }

    fn command_custom(
        &self,
        context: &mut Self::ExecutionContext,
        state: &mut Self::State,
        cmd: &Command,
    ) -> bool {
        use Command_ as C;

        match &cmd.value {
            C::JumpIf {
                cond,
                if_true,
                if_false,
            } => {
                self.exp(context, state, cond);

                let true_is_abort = self.exit_blocks.contains(if_true);
                let false_is_abort = self.exit_blocks.contains(if_false);

                if true_is_abort ^ false_is_abort {
                    let cond_true_on_continue = false_is_abort;
                    self.track_zero_guard(state, cond, cond_true_on_continue);
                }

                true
            }
            _ => false,
        }
    }

    fn exp_custom(
        &self,
        context: &mut Self::ExecutionContext,
        state: &mut Self::State,
        e: &Exp,
    ) -> Option<Vec<ZeroCheckValue>> {
        use UnannotatedExp_ as E;

        match &e.exp.value {
            E::Value(v) => {
                let is_zero = v.value.is_zero();
                return Some(vec![if is_zero {
                    ZeroCheckValue::ConstantZero
                } else {
                    ZeroCheckValue::Unknown
                }]);
            }
            E::Constant(_) => return Some(vec![ZeroCheckValue::Unknown]),
            E::ModuleCall(call) => {
                let module_sym = call.module.value.module.value();
                let func_sym = call.name.value();
                let module_name = module_sym.as_str();
                let func_name = func_sym.as_str();

                // Check for destroy_zero calls
                if (module_name == "balance" || module_name == "coin")
                    && func_name == "destroy_zero"
                    && let Some(arg) = call.arguments.first()
                {
                    let is_checked = self.is_zero_checked(state, arg);
                    if !is_checked && self.is_root_source_loc(&e.exp.loc) {
                        let msg = "destroy_zero called without verifying value is zero first";
                        let help = "Add validation: `assert!(balance::value(&b) == 0, E_NOT_ZERO)`";
                        let d = diag!(
                            DESTROY_ZERO_UNCHECKED_V2_DIAG,
                            (e.exp.loc, msg),
                            (arg.exp.loc, help),
                        );
                        context.add_diag(d);
                    }
                }

                // Check for assert! calls to track zero guards
                if self.is_assert_call(call)
                    && let Some(cond) = call.arguments.first()
                {
                    self.track_zero_guard(state, cond, true);
                }
            }
            _ => {}
        }

        None
    }
}

impl DestroyZeroVerifierAI<'_> {
    fn is_root_source_loc(&self, _loc: &Loc) -> bool {
        // Check if this is root package code (not a dependency)
        let is_dependency = self
            .context
            .env
            .package_config(self.context.package)
            .is_dependency;
        !is_dependency
    }

    fn is_zero_checked(&self, state: &DestroyZeroState, expr: &Exp) -> bool {
        match &expr.exp.value {
            UnannotatedExp_::Move { var, .. }
            | UnannotatedExp_::Copy { var, .. }
            | UnannotatedExp_::BorrowLocal(_, var) => matches!(
                state.locals.get(var),
                Some(LocalState::Available(
                    _,
                    ZeroCheckValue::CheckedZero | ZeroCheckValue::ConstantZero
                ))
            ),
            UnannotatedExp_::Value(v) => v.value.is_zero(),
            _ => false,
        }
    }

    fn is_assert_call(&self, call: &ModuleCall) -> bool {
        let func_sym = call.name.value();
        let func_name = func_sym.as_str();
        func_name == "assert" || func_name.contains("assert")
    }

    fn track_zero_guard(&self, state: &mut DestroyZeroState, condition: &Exp, cond_value: bool) {
        if let Some((var, zero_when_true)) = self.extract_zero_guard(condition) {
            let implies_zero = if cond_value {
                zero_when_true
            } else {
                !zero_when_true
            };

            if implies_zero {
                self.mark_zero_checked(state, var);
            }
        }
    }

    fn extract_zero_guard(&self, condition: &Exp) -> Option<(Var, bool)> {
        match &condition.exp.value {
            UnannotatedExp_::UnaryExp(unop, inner)
                if matches!(unop.value, move_compiler::parser::ast::UnaryOp_::Not) =>
            {
                self.extract_zero_guard(inner)
                    .map(|(v, zero_when_true)| (v, !zero_when_true))
            }
            UnannotatedExp_::BinopExp(lhs, op, rhs) => {
                // Handle `a && b`
                if matches!(op.value, BinOp_::And) {
                    return self
                        .extract_zero_guard(lhs)
                        .or_else(|| self.extract_zero_guard(rhs));
                }

                // Check for `value == 0` or `0 == value` patterns
                let is_zero_literal = |e: &Exp| -> bool {
                    matches!(&e.exp.value, UnannotatedExp_::Value(v) if v.value.is_zero())
                };

                let (var_side, _is_zero_side, _var_on_lhs) = if is_zero_literal(rhs) {
                    (lhs, true, true)
                } else if is_zero_literal(lhs) {
                    (rhs, true, false)
                } else {
                    return None;
                };

                let var = self
                    .extract_var(var_side)
                    .or_else(|| self.extract_var_from_value_call(var_side))?;

                // `var == 0` means var is zero when true
                // `var != 0` means var is NOT zero when true
                let zero_when_true = match op.value {
                    BinOp_::Eq => true,
                    BinOp_::Neq => false,
                    _ => return None,
                };

                Some((var, zero_when_true))
            }
            // Handle value() calls - `value(&b) == 0`
            UnannotatedExp_::ModuleCall(call) => {
                let func_sym = call.name.value();
                let func_name = func_sym.as_str();
                if func_name == "value"
                    && let Some(arg) = call.arguments.first()
                {
                    return self.extract_var(arg).map(|v| (v, false)); // Conservative
                }
                None
            }
            _ => None,
        }
    }

    fn extract_var_from_value_call(&self, e: &Exp) -> Option<Var> {
        match &e.exp.value {
            UnannotatedExp_::ModuleCall(call) => {
                let module_sym = call.module.value.module.value();
                let func_sym = call.name.value();
                let module_name = module_sym.as_str();
                let func_name = func_sym.as_str();
                if (module_name == "balance" || module_name == "coin")
                    && func_name == "value"
                    && let Some(arg) = call.arguments.first()
                {
                    self.extract_var(arg)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn extract_var(&self, e: &Exp) -> Option<Var> {
        match &e.exp.value {
            UnannotatedExp_::Move { var, .. }
            | UnannotatedExp_::Copy { var, .. }
            | UnannotatedExp_::BorrowLocal(_, var) => Some(*var),
            _ => None,
        }
    }

    fn mark_zero_checked(&self, state: &mut DestroyZeroState, var: Var) {
        if let Some(LocalState::Available(loc, val)) = state.locals.get_mut(&var) {
            *val = ZeroCheckValue::CheckedZero;
        }
    }
}

impl SimpleDomain for DestroyZeroState {
    type Value = ZeroCheckValue;

    fn new(context: &CFGContext, locals: BTreeMap<Var, LocalState<Self::Value>>) -> Self {
        DestroyZeroState { locals }
    }

    fn locals_mut(&mut self) -> &mut BTreeMap<Var, LocalState<Self::Value>> {
        &mut self.locals
    }

    fn locals(&self) -> &BTreeMap<Var, LocalState<Self::Value>> {
        &self.locals
    }

    fn join_value(v1: &Self::Value, v2: &Self::Value) -> Self::Value {
        use ZeroCheckValue::*;
        // Pessimistic: if either path didn't check, result is unknown
        match (v1, v2) {
            (CheckedZero, CheckedZero) => CheckedZero,
            (ConstantZero, ConstantZero) => ConstantZero,
            (CheckedZero, ConstantZero) | (ConstantZero, CheckedZero) => CheckedZero,
            _ => Unknown,
        }
    }

    fn join_impl(&mut self, _other: &Self, _result: &mut JoinResult) {
        // No additional state to join beyond locals
    }
}

impl SimpleExecutionContext for DestroyZeroExecutionContext {
    fn add_diag(&mut self, d: CompilerDiagnostic) {
        self.diags.add(d);
    }
}

// ============================================================================
// 4. Fresh Address Reuse V2 (CFG-aware)
// ============================================================================

const FRESH_ADDRESS_REUSE_V2_DIAG: DiagnosticInfo = custom(
    LINT_WARNING_PREFIX,
    Severity::Warning,
    CLIPPY_CATEGORY,
    5, // fresh_address_reuse_v2
    "fresh_object_address result reused",
);

pub static FRESH_ADDRESS_REUSE_V2: LintDescriptor = LintDescriptor {
    name: "fresh_address_reuse_v2",
    category: LintCategory::Security,
    description: "fresh_object_address result used multiple times (CFG-aware, requires --mode full --preview)",
    group: RuleGroup::Preview,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBasedCFG,
    gap: Some(TypeSystemGap::OwnershipViolation),
};

pub struct FreshAddressReuseVerifier;

pub struct FreshAddressReuseVerifierAI<'a> {
    info: &'a TypingProgramInfo,
    context: &'a CFGContext<'a>,
}

/// Abstract value for tracking fresh address usage
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum FreshAddressValue {
    /// Not a fresh address
    #[default]
    NotFresh,
    /// Fresh address, not yet used
    Fresh(Loc),
    /// Fresh address, already used once (unsafe to reuse)
    UsedOnce(Loc),
}

pub struct FreshAddressExecutionContext {
    diags: CompilerDiagnostics,
}

#[derive(Clone, Debug)]
pub struct FreshAddressState {
    locals: BTreeMap<Var, LocalState<FreshAddressValue>>,
}

impl SimpleAbsIntConstructor for FreshAddressReuseVerifier {
    type AI<'a> = FreshAddressReuseVerifierAI<'a>;

    fn new<'a>(
        context: &'a CFGContext<'a>,
        _cfg: &ImmForwardCFG,
        _init_state: &mut FreshAddressState,
    ) -> Option<Self::AI<'a>> {
        if context.attributes.is_test_or_test_only() {
            return None;
        }

        Some(FreshAddressReuseVerifierAI {
            info: context.info,
            context,
        })
    }
}

impl SimpleAbsInt for FreshAddressReuseVerifierAI<'_> {
    type State = FreshAddressState;
    type ExecutionContext = FreshAddressExecutionContext;

    fn finish(
        &mut self,
        _final_states: BTreeMap<Label, Self::State>,
        diags: CompilerDiagnostics,
    ) -> CompilerDiagnostics {
        diags
    }

    fn start_command(&self, _pre: &mut Self::State) -> Self::ExecutionContext {
        FreshAddressExecutionContext {
            diags: CompilerDiagnostics::new(),
        }
    }

    fn finish_command(
        &self,
        context: Self::ExecutionContext,
        _state: &mut Self::State,
    ) -> CompilerDiagnostics {
        context.diags
    }

    fn exp_custom(
        &self,
        context: &mut Self::ExecutionContext,
        state: &mut Self::State,
        e: &Exp,
    ) -> Option<Vec<FreshAddressValue>> {
        use UnannotatedExp_ as E;

        if let E::ModuleCall(call) = &e.exp.value {
            let module_sym = call.module.value.module.value();
            let func_sym = call.name.value();
            let module_name = module_sym.as_str();
            let func_name = func_sym.as_str();

            // Track fresh_object_address calls
            if module_name == "tx_context" && func_name == "fresh_object_address" {
                return Some(vec![FreshAddressValue::Fresh(e.exp.loc)]);
            }

            // Check for new_uid_from_address calls
            if module_name == "object"
                && func_name == "new_uid_from_address"
                && let Some(arg) = call.arguments.first()
                && let Some(var) = self.extract_var(arg)
            {
                // First extract what we need from the immutable borrow
                let action = if let Some(LocalState::Available(_, val)) = state.locals.get(&var) {
                    match val {
                        FreshAddressValue::UsedOnce(original_loc) => {
                            // Already used - this is a reuse!
                            Some(("reuse", *original_loc))
                        }
                        FreshAddressValue::Fresh(loc) => {
                            // First use - need to mark as used
                            Some(("first_use", *loc))
                        }
                        _ => None,
                    }
                } else {
                    None
                };

                // Now handle the action without holding the borrow
                if let Some((action_type, loc)) = action {
                    if action_type == "reuse" {
                        if self.is_root_source_loc(&e.exp.loc) {
                            let msg = "fresh_object_address result is being reused - each UID needs its own fresh address";
                            let help = "Use `object::new(ctx)` instead, or call `fresh_object_address` again";
                            let d =
                                diag!(FRESH_ADDRESS_REUSE_V2_DIAG, (e.exp.loc, msg), (loc, help),);
                            context.add_diag(d);
                        }
                    } else if action_type == "first_use" {
                        // Mark as used
                        if let Some(LocalState::Available(_, v)) = state.locals.get_mut(&var) {
                            *v = FreshAddressValue::UsedOnce(loc);
                        }
                    }
                }
            }
        }

        None
    }
}

impl FreshAddressReuseVerifierAI<'_> {
    fn is_root_source_loc(&self, _loc: &Loc) -> bool {
        // Check if this is root package code (not a dependency)
        let is_dependency = self
            .context
            .env
            .package_config(self.context.package)
            .is_dependency;
        !is_dependency
    }

    fn extract_var(&self, e: &Exp) -> Option<Var> {
        match &e.exp.value {
            UnannotatedExp_::Move { var, .. }
            | UnannotatedExp_::Copy { var, .. }
            | UnannotatedExp_::BorrowLocal(_, var) => Some(*var),
            _ => None,
        }
    }
}

impl SimpleDomain for FreshAddressState {
    type Value = FreshAddressValue;

    fn new(context: &CFGContext, locals: BTreeMap<Var, LocalState<Self::Value>>) -> Self {
        FreshAddressState { locals }
    }

    fn locals_mut(&mut self) -> &mut BTreeMap<Var, LocalState<Self::Value>> {
        &mut self.locals
    }

    fn locals(&self) -> &BTreeMap<Var, LocalState<Self::Value>> {
        &self.locals
    }

    fn join_value(v1: &Self::Value, v2: &Self::Value) -> Self::Value {
        use FreshAddressValue::*;
        // Pessimistic: if any path has used it, it's used
        match (v1, v2) {
            (UsedOnce(loc), _) | (_, UsedOnce(loc)) => UsedOnce(*loc),
            (Fresh(loc), _) | (_, Fresh(loc)) => Fresh(*loc),
            _ => NotFresh,
        }
    }

    fn join_impl(&mut self, _other: &Self, _result: &mut JoinResult) {
        // No additional state to join beyond locals
    }
}

impl SimpleExecutionContext for FreshAddressExecutionContext {
    fn add_diag(&mut self, d: CompilerDiagnostic) {
        self.diags.add(d);
    }
}

// ============================================================================
// 5. REMOVED: Tainted Transfer Recipient
// ============================================================================
// Deprecated and removed - 100% false positive rate.
// The signer IS the authority in Sui's ownership model; passing an address
// parameter to transfer is the intended pattern, not a security issue.

// ============================================================================
// 6. Capability Escape Analysis (CFG-aware)
// ============================================================================
//
// Detects when capability objects (key+store, no copy/drop) escape to
// potentially unauthorized contexts:
// - Transferred to address from function parameter (suspicious)
// - Passed to external module function (dangerous)
// - Stored in shared object (dangerous)
//
// Safe escapes:
// - Returned to caller
// - Transferred to fixed/constant address
// - Stored in owned object

const CAPABILITY_ESCAPE_DIAG: DiagnosticInfo = custom(
    LINT_WARNING_PREFIX,
    Severity::Warning,
    CLIPPY_CATEGORY,
    7, // capability_escape
    "capability object escapes to potentially unauthorized context",
);

pub static CAPABILITY_ESCAPE: LintDescriptor = LintDescriptor {
    name: "capability_escape",
    category: LintCategory::Security,
    description: "Capability object escapes to potentially unauthorized context (CFG-aware, requires --mode full --experimental)",
    group: RuleGroup::Experimental,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBasedCFG,
    gap: Some(TypeSystemGap::CapabilityEscape),
};

pub struct CapabilityEscapeVerifier;

pub struct CapabilityEscapeVerifierAI<'a> {
    cap_vars: Vec<(Var, Loc)>,
    addr_params: BTreeSet<Var>,
    info: &'a TypingProgramInfo,
    context: &'a CFGContext<'a>,
    escapes: RefCell<Vec<(Loc, String, EscapeKind)>>,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EscapeKind {
    ToParameterAddress,
    ToExternalModule,
    ToSharedObject,
    UnconditionalTransfer,
}

impl EscapeKind {
    fn severity(&self) -> &'static str {
        match self {
            EscapeKind::ToParameterAddress => "suspicious",
            EscapeKind::ToExternalModule => "dangerous",
            EscapeKind::ToSharedObject => "dangerous",
            EscapeKind::UnconditionalTransfer => "warning",
        }
    }

    fn description(&self) -> &'static str {
        match self {
            EscapeKind::ToParameterAddress => {
                "capability transferred to address from function parameter"
            }
            EscapeKind::ToExternalModule => "capability passed to external module function",
            EscapeKind::ToSharedObject => "capability stored in shared object",
            EscapeKind::UnconditionalTransfer => {
                "capability transferred without authorization check"
            }
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum EscapeValue {
    #[default]
    NotTracked,
    Contained(Loc),
    Validated(Loc),
    EscapedSafe(Loc),
    EscapedSuspicious(Loc),
    EscapedDangerous(Loc),
}

pub struct EscapeExecutionContext {
    diags: CompilerDiagnostics,
}

#[derive(Clone, Debug)]
pub struct EscapeState {
    locals: BTreeMap<Var, LocalState<EscapeValue>>,
    validated_caps: BTreeSet<String>,
}

impl SimpleAbsIntConstructor for CapabilityEscapeVerifier {
    type AI<'a> = CapabilityEscapeVerifierAI<'a>;

    fn new<'a>(
        context: &'a CFGContext<'a>,
        _cfg: &ImmForwardCFG,
        init_state: &mut EscapeState,
    ) -> Option<Self::AI<'a>> {
        if context.attributes.is_test_or_test_only() {
            return None;
        }

        let mut cap_vars: Vec<(Var, Loc)> = Vec::new();
        let mut addr_params: BTreeSet<Var> = BTreeSet::new();

        for (_, var, ty) in &context.signature.parameters {
            if is_address_type_escape(ty) {
                addr_params.insert(*var);
            }
            if is_capability_by_value_escape(ty) {
                cap_vars.push((*var, var.0.loc));
                if let Some(LocalState::Available(_, value)) = init_state.locals.get_mut(var) {
                    *value = EscapeValue::Contained(var.0.loc);
                }
            }
        }

        if cap_vars.is_empty() {
            return None;
        }

        Some(CapabilityEscapeVerifierAI {
            cap_vars,
            addr_params,
            info: context.info,
            context,
            escapes: RefCell::new(Vec::new()),
        })
    }
}

impl SimpleAbsInt for CapabilityEscapeVerifierAI<'_> {
    type State = EscapeState;
    type ExecutionContext = EscapeExecutionContext;

    fn finish(
        &mut self,
        _final_states: BTreeMap<Label, Self::State>,
        mut diags: CompilerDiagnostics,
    ) -> CompilerDiagnostics {
        for (loc, cap_name, kind) in self.escapes.borrow().iter() {
            let msg = format!(
                "Capability `{}` {}: {}",
                cap_name,
                kind.severity(),
                kind.description()
            );
            let help = match kind {
                EscapeKind::ToParameterAddress => {
                    "Consider requiring an authorization capability parameter to validate the recipient"
                }
                EscapeKind::ToExternalModule => {
                    "Ensure the external module is trusted, or add authorization before the call"
                }
                EscapeKind::ToSharedObject => {
                    "Storing capabilities in shared objects allows anyone to access them"
                }
                EscapeKind::UnconditionalTransfer => {
                    "Add an authorization check (e.g., assert with capability validation) before transfer"
                }
            };
            diags.add(diag!(CAPABILITY_ESCAPE_DIAG, (*loc, msg), (*loc, help)));
        }
        diags
    }

    fn start_command(&self, _pre: &mut Self::State) -> Self::ExecutionContext {
        EscapeExecutionContext {
            diags: CompilerDiagnostics::new(),
        }
    }

    fn finish_command(
        &self,
        context: Self::ExecutionContext,
        _state: &mut Self::State,
    ) -> CompilerDiagnostics {
        context.diags
    }

    fn command_custom(
        &self,
        context: &mut Self::ExecutionContext,
        state: &mut Self::State,
        cmd: &Command,
    ) -> bool {
        match &cmd.value {
            Command_::JumpIf { cond, .. } => {
                self.exp(context, state, cond);
                self.track_validation_in_condition(state, cond);
                true
            }
            _ => false,
        }
    }

    fn exp_custom(
        &self,
        context: &mut Self::ExecutionContext,
        state: &mut Self::State,
        e: &Exp,
    ) -> Option<Vec<EscapeValue>> {
        use UnannotatedExp_ as E;

        if let E::ModuleCall(call) = &e.exp.value {
            if self.is_transfer_call(call) {
                self.check_transfer_escape(state, call, e.exp.loc);
            } else if self.is_share_object_call(call) {
                self.check_share_escape(state, call, e.exp.loc);
            } else if self.is_external_module_call(call) {
                self.check_external_call_escape(state, call, e.exp.loc);
            }
        }
        None
    }
}

impl CapabilityEscapeVerifierAI<'_> {
    fn is_tracked_cap(&self, var: &Var) -> bool {
        self.cap_vars.iter().any(|(v, _)| v.value() == var.value())
    }

    fn get_cap_name(&self, var: &Var) -> String {
        var.value().as_str().to_owned()
    }

    fn is_addr_param(&self, var: &Var) -> bool {
        self.addr_params.contains(var)
    }

    fn is_validated(&self, state: &EscapeState, var: &Var) -> bool {
        state.validated_caps.contains(&self.get_cap_name(var))
    }

    fn is_root_source_loc(&self) -> bool {
        !self
            .context
            .env
            .package_config(self.context.package)
            .is_dependency
    }

    fn track_validation_in_condition(&self, state: &mut EscapeState, cond: &Exp) {
        let accessed = self.extract_capability_accesses(cond);
        for (var, _) in accessed {
            if self.is_tracked_cap(&var) {
                state.validated_caps.insert(self.get_cap_name(&var));
            }
        }
    }

    fn extract_capability_accesses(&self, exp: &Exp) -> Vec<(Var, Loc)> {
        let mut accesses = Vec::new();
        self.collect_cap_accesses(exp, &mut accesses);
        accesses
    }

    fn collect_cap_accesses(&self, exp: &Exp, accesses: &mut Vec<(Var, Loc)>) {
        use UnannotatedExp_ as E;
        match &exp.exp.value {
            E::Borrow(_, inner, _, _) => {
                if let E::BorrowLocal(_, var) | E::Copy { var, .. } | E::Move { var, .. } =
                    &inner.exp.value
                {
                    if self.is_tracked_cap(var) {
                        accesses.push((*var, exp.exp.loc));
                    }
                }
                self.collect_cap_accesses(inner, accesses);
            }
            E::BorrowLocal(_, var) | E::Copy { var, .. } | E::Move { var, .. } => {
                if self.is_tracked_cap(var) {
                    accesses.push((*var, exp.exp.loc));
                }
            }
            E::BinopExp(lhs, _, rhs) => {
                self.collect_cap_accesses(lhs, accesses);
                self.collect_cap_accesses(rhs, accesses);
            }
            E::UnaryExp(_, inner)
            | E::Dereference(inner)
            | E::Freeze(inner)
            | E::Cast(inner, _) => {
                self.collect_cap_accesses(inner, accesses);
            }
            E::ModuleCall(call) => {
                for arg in &call.arguments {
                    self.collect_cap_accesses(arg, accesses);
                }
            }
            E::Vector(_, _, _, args) => {
                for arg in args {
                    self.collect_cap_accesses(arg, accesses);
                }
            }
            E::Multiple(es) => {
                for e in es {
                    self.collect_cap_accesses(e, accesses);
                }
            }
            _ => {}
        }
    }

    fn is_transfer_call(&self, call: &ModuleCall) -> bool {
        call.is(&SUI_ADDR, "transfer", "transfer")
            || call.is(&SUI_ADDR, "transfer", "public_transfer")
    }

    fn is_share_object_call(&self, call: &ModuleCall) -> bool {
        call.is(&SUI_ADDR, "transfer", "share_object")
            || call.is(&SUI_ADDR, "transfer", "public_share_object")
    }

    fn is_external_module_call(&self, call: &ModuleCall) -> bool {
        // Check if call is to a different module than the current one
        let call_module = &call.module.value;
        let current_module = &self.context.module.value;

        // Same module = not external
        if call_module == current_module {
            return false;
        }

        // Check if it's sui framework (trusted)
        let is_framework = match &call_module.address {
            move_compiler::expansion::ast::Address::Numerical { value, .. } => {
                is_sui_framework_addr_escape(&value.value)
            }
            move_compiler::expansion::ast::Address::NamedUnassigned(_) => false,
        };

        if is_framework {
            return false;
        }

        self.has_capability_argument(call)
    }

    fn has_capability_argument(&self, call: &ModuleCall) -> bool {
        for arg in &call.arguments {
            if let Some(var) = self.extract_var(arg) {
                if self.is_tracked_cap(&var) {
                    return true;
                }
            }
        }
        false
    }

    fn check_transfer_escape(&self, state: &EscapeState, call: &ModuleCall, loc: Loc) {
        if !self.is_root_source_loc() {
            return;
        }
        if call.arguments.len() < 2 {
            return;
        }

        let cap_arg = &call.arguments[0];
        let recipient_arg = &call.arguments[1];

        let cap_var = match self.extract_var(cap_arg) {
            Some(v) if self.is_tracked_cap(&v) => v,
            _ => return,
        };

        if let Some(recipient_var) = self.extract_var(recipient_arg) {
            if self.is_addr_param(&recipient_var) && !self.is_validated(state, &cap_var) {
                self.escapes.borrow_mut().push((
                    loc,
                    self.get_cap_name(&cap_var),
                    EscapeKind::ToParameterAddress,
                ));
                return;
            }
        }

        if self.is_constant_address(recipient_arg) {
            return;
        }

        if !self.is_validated(state, &cap_var) {
            self.escapes.borrow_mut().push((
                loc,
                self.get_cap_name(&cap_var),
                EscapeKind::UnconditionalTransfer,
            ));
        }
    }

    fn check_share_escape(&self, state: &EscapeState, call: &ModuleCall, loc: Loc) {
        if !self.is_root_source_loc() {
            return;
        }
        for arg in &call.arguments {
            if let Some(var) = self.extract_var(arg) {
                if self.is_tracked_cap(&var)
                    && matches!(&arg.exp.value, UnannotatedExp_::Move { .. })
                {
                    self.escapes.borrow_mut().push((
                        loc,
                        self.get_cap_name(&var),
                        EscapeKind::ToSharedObject,
                    ));
                }
            }
        }
    }

    fn check_external_call_escape(&self, state: &EscapeState, call: &ModuleCall, loc: Loc) {
        if !self.is_root_source_loc() {
            return;
        }
        for arg in &call.arguments {
            if let Some(var) = self.extract_var(arg) {
                if self.is_tracked_cap(&var)
                    && matches!(&arg.exp.value, UnannotatedExp_::Move { .. })
                {
                    self.escapes.borrow_mut().push((
                        loc,
                        self.get_cap_name(&var),
                        EscapeKind::ToExternalModule,
                    ));
                }
            }
        }
    }

    fn extract_var(&self, e: &Exp) -> Option<Var> {
        match &e.exp.value {
            UnannotatedExp_::Move { var, .. }
            | UnannotatedExp_::Copy { var, .. }
            | UnannotatedExp_::BorrowLocal(_, var) => Some(*var),
            _ => None,
        }
    }

    fn is_constant_address(&self, e: &Exp) -> bool {
        matches!(
            &e.exp.value,
            UnannotatedExp_::Value(_) | UnannotatedExp_::Constant(_)
        )
    }
}

fn is_address_type_escape(ty: &SingleType) -> bool {
    match &ty.value {
        SingleType_::Base(bt) | SingleType_::Ref(_, bt) => {
            if let BaseType_::Apply(_, type_name, _) = &bt.value {
                if let TypeName_::Builtin(b) = &type_name.value {
                    return matches!(b.value, BuiltinTypeName_::Address);
                }
            }
            false
        }
    }
}

fn is_capability_by_value_escape(ty: &SingleType) -> bool {
    match &ty.value {
        SingleType_::Base(bt) => is_capability_base_type_escape(&bt.value),
        SingleType_::Ref(_, _) => false,
    }
}

fn is_capability_base_type_escape(bt: &BaseType_) -> bool {
    matches!(bt, BaseType_::Apply(abilities, _, _)
        if abilities.has_ability_(Ability_::Key)
            && abilities.has_ability_(Ability_::Store)
            && !abilities.has_ability_(Ability_::Copy)
            && !abilities.has_ability_(Ability_::Drop))
}

fn is_sui_framework_addr_escape(addr: &NumericalAddress) -> bool {
    let bytes = addr.into_inner();
    bytes[..31].iter().all(|&b| b == 0) && (bytes[31] == 1 || bytes[31] == 2)
}

impl SimpleDomain for EscapeState {
    type Value = EscapeValue;

    fn new(_context: &CFGContext, locals: BTreeMap<Var, LocalState<Self::Value>>) -> Self {
        EscapeState {
            locals,
            validated_caps: BTreeSet::new(),
        }
    }

    fn locals_mut(&mut self) -> &mut BTreeMap<Var, LocalState<Self::Value>> {
        &mut self.locals
    }

    fn locals(&self) -> &BTreeMap<Var, LocalState<Self::Value>> {
        &self.locals
    }

    fn join_value(v1: &Self::Value, v2: &Self::Value) -> Self::Value {
        use EscapeValue::*;
        match (v1, v2) {
            (EscapedDangerous(loc), _) | (_, EscapedDangerous(loc)) => EscapedDangerous(*loc),
            (EscapedSuspicious(loc), _) | (_, EscapedSuspicious(loc)) => EscapedSuspicious(*loc),
            (EscapedSafe(loc), _) | (_, EscapedSafe(loc)) => EscapedSafe(*loc),
            (Validated(loc), _) | (_, Validated(loc)) => Validated(*loc),
            (Contained(loc), _) | (_, Contained(loc)) => Contained(*loc),
            (NotTracked, NotTracked) => NotTracked,
        }
    }

    fn join_impl(&mut self, other: &Self, _result: &mut JoinResult) {
        for cap in &other.validated_caps {
            self.validated_caps.insert(cap.clone());
        }
    }
}

impl SimpleExecutionContext for EscapeExecutionContext {
    fn add_diag(&mut self, d: CompilerDiagnostic) {
        self.diags.add(d);
    }
}

// ============================================================================
// 7. Stale Oracle Price V3 (CFG-aware dataflow analysis)
// ============================================================================

/// Verifies that oracle prices from `get_price_unsafe` are validated for freshness
/// before being used in business logic.
///
/// Tracks Price values through the CFG and reports when they are used without
/// passing through a freshness validation function.
pub struct StaleOraclePriceVerifier;

pub struct StaleOraclePriceVerifierAI<'a> {
    info: &'a TypingProgramInfo,
    context: &'a CFGContext<'a>,
    /// Labels whose blocks immediately exit (abort/return)
    exit_blocks: BTreeSet<Label>,
}

/// Abstract value: tracks whether a Price has been freshness-validated
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum PriceValidationValue {
    /// Not a tracked price value
    #[default]
    NotTracked,
    /// Price from get_price_unsafe, not yet validated
    Unvalidated(Loc),
    /// Price has been validated for freshness
    Validated(Loc),
}

pub struct PriceExecutionContext {
    diags: CompilerDiagnostics,
}

#[derive(Clone, Debug, Default)]
pub struct PriceState {
    locals: BTreeMap<Var, LocalState<PriceValidationValue>>,
}

/// Oracle modules and their unsafe price functions
const UNSAFE_ORACLE_CALLS: &[(&str, &str)] = &[
    ("pyth", "get_price_unsafe"),
    ("price_info", "get_price_unsafe"),
    ("switchboard", "get_price_unsafe"),
    ("supra", "get_price_unsafe"),
];

/// Functions that validate price freshness
const FRESHNESS_VALIDATION_CALLS: &[&str] = &[
    "check_price_is_fresh",
    "check_freshness",
    "get_price_no_older_than",
    "validate_freshness",
    "assert_fresh",
];

impl SimpleAbsIntConstructor for StaleOraclePriceVerifier {
    type AI<'a> = StaleOraclePriceVerifierAI<'a>;

    fn new<'a>(
        context: &'a CFGContext<'a>,
        cfg: &ImmForwardCFG,
        _init_state: &mut PriceState,
    ) -> Option<Self::AI<'a>> {
        // Skip test functions
        if context.attributes.is_test_or_test_only() {
            return None;
        }

        // Precompute exit blocks for guard detection
        let mut exit_blocks = BTreeSet::new();
        for lbl in cfg.block_labels() {
            if is_immediate_exit_block(cfg, lbl) {
                exit_blocks.insert(lbl);
            }
        }

        Some(StaleOraclePriceVerifierAI {
            info: context.info,
            context,
            exit_blocks,
        })
    }
}

impl SimpleAbsInt for StaleOraclePriceVerifierAI<'_> {
    type State = PriceState;
    type ExecutionContext = PriceExecutionContext;

    fn finish(
        &mut self,
        _final_states: BTreeMap<Label, Self::State>,
        diags: CompilerDiagnostics,
    ) -> CompilerDiagnostics {
        diags
    }

    fn start_command(&self, _pre: &mut Self::State) -> Self::ExecutionContext {
        PriceExecutionContext {
            diags: CompilerDiagnostics::new(),
        }
    }

    fn finish_command(
        &self,
        context: Self::ExecutionContext,
        _state: &mut Self::State,
    ) -> CompilerDiagnostics {
        context.diags
    }

    fn command_custom(
        &self,
        context: &mut Self::ExecutionContext,
        state: &mut Self::State,
        cmd: &Command,
    ) -> bool {
        use Command_ as C;

        match &cmd.value {
            C::Assign(_case, lvalues, rhs) => {
                // Evaluate RHS to get its price validation state
                let rhs_values = self.exp(context, state, rhs);

                // Find if any RHS value is unvalidated
                let rhs_unvalidated = rhs_values
                    .iter()
                    .find(|v| matches!(v, PriceValidationValue::Unvalidated(_)));

                // Propagate to LHS variables
                for lvalue in lvalues {
                    if let LValue_::Var { var, .. } = &lvalue.value {
                        if let Some(PriceValidationValue::Unvalidated(source_loc)) = rhs_unvalidated
                        {
                            // Mark variable as holding unvalidated price
                            if let Some(local_state) = state.locals.get_mut(var) {
                                if let LocalState::Available(avail_loc, _) = local_state {
                                    *local_state = LocalState::Available(
                                        *avail_loc,
                                        PriceValidationValue::Unvalidated(*source_loc),
                                    );
                                }
                            }
                        }
                    }
                }

                false // Let default handling continue
            }
            C::JumpIf {
                cond,
                if_true,
                if_false,
            } => {
                // Visit condition
                self.exp(context, state, cond);

                // Check if one branch is an exit (abort) - this is a guard pattern
                let true_is_abort = self.exit_blocks.contains(if_true);
                let false_is_abort = self.exit_blocks.contains(if_false);

                // If this is a guard pattern with freshness check, mark prices as validated
                if true_is_abort || false_is_abort {
                    self.mark_validated_from_guard(state, cond);
                }

                true // Handled
            }
            _ => false,
        }
    }

    fn exp_custom(
        &self,
        context: &mut Self::ExecutionContext,
        state: &mut Self::State,
        e: &Exp,
    ) -> Option<Vec<PriceValidationValue>> {
        use UnannotatedExp_ as E;

        match &e.exp.value {
            // Variable access - return tracked state
            E::Move { var, .. } | E::Copy { var, .. } | E::BorrowLocal(_, var) => {
                if let Some(LocalState::Available(_, value)) = state.locals.get(var) {
                    return Some(vec![*value]);
                }
            }
            // Module call - detect sources and handle validation
            E::ModuleCall(call) => {
                let module_sym = call.module.value.module.value();
                let module_name = module_sym.as_str();
                let func_sym = call.name.value();
                let func_name = func_sym.as_str();

                // Check if this is an unsafe oracle call (SOURCE)
                let is_unsafe = UNSAFE_ORACLE_CALLS
                    .iter()
                    .any(|(m, f)| module_name == *m && func_name == *f);

                if is_unsafe {
                    // Mark the return value as unvalidated
                    return Some(vec![PriceValidationValue::Unvalidated(e.exp.loc)]);
                }

                // Check if this is a freshness validation call
                let is_validation = FRESHNESS_VALIDATION_CALLS
                    .iter()
                    .any(|f| func_name.contains(f));

                if is_validation {
                    // Process arguments - any unvalidated price passed here becomes validated
                    for arg in &call.arguments {
                        let arg_values = self.exp(context, state, arg);
                        // Mark any variables passed as validated in state
                        if let Some(var) = self.extract_var(arg) {
                            if let Some(LocalState::Available(avail_loc, _)) =
                                state.locals.get(&var)
                            {
                                state.locals.insert(
                                    var,
                                    LocalState::Available(
                                        *avail_loc,
                                        PriceValidationValue::Validated(e.exp.loc),
                                    ),
                                );
                            }
                        }
                    }
                    return Some(vec![PriceValidationValue::Validated(e.exp.loc)]);
                }

                // For other module calls, let call_custom handle sink detection
                return None; // Use default handling which will call call_custom
            }
            _ => {}
        }

        None
    }

    fn call_custom(
        &self,
        context: &mut Self::ExecutionContext,
        state: &mut Self::State,
        loc: &Loc,
        _return_ty: &Type,
        call: &ModuleCall,
        args: Vec<PriceValidationValue>,
    ) -> Option<Vec<PriceValidationValue>> {
        let module_sym = call.module.value.module.value();
        let module_name = module_sym.as_str();
        let func_sym = call.name.value();
        let func_name = func_sym.as_str();

        // Skip if this is an oracle call or validation call (handled in exp_custom)
        let is_oracle = UNSAFE_ORACLE_CALLS
            .iter()
            .any(|(m, f)| module_name == *m && func_name == *f);
        let is_validation = FRESHNESS_VALIDATION_CALLS
            .iter()
            .any(|f| func_name.contains(f));

        if is_oracle || is_validation {
            return None;
        }

        // Check if any argument is an unvalidated price (SINK detection)
        for (idx, val) in args.iter().enumerate() {
            if let PriceValidationValue::Unvalidated(source_loc) = val {
                // Don't report if this is in dependency code
                if self.is_root_source_loc(loc) {
                    let msg = format!(
                        "Unvalidated oracle price from `get_price_unsafe` passed to `{}::{}`. \
                         The price may be stale. Consider validating freshness first with \
                         `check_price_is_fresh` or use `get_price_no_older_than`.",
                        module_name, func_name
                    );
                    let help = "Add freshness validation before using the price";
                    let d = diag!(STALE_ORACLE_PRICE_V3_DIAG, (*loc, msg), (*source_loc, help),);
                    context.add_diag(d);
                }
            }
        }

        None
    }
}

impl StaleOraclePriceVerifierAI<'_> {
    fn is_root_source_loc(&self, loc: &Loc) -> bool {
        let is_dependency = self
            .context
            .env
            .package_config(self.context.package)
            .is_dependency;
        !is_dependency
    }

    /// Extract variable from an expression (for tracking through references)
    fn extract_var(&self, e: &Exp) -> Option<Var> {
        use UnannotatedExp_ as E;
        match &e.exp.value {
            E::Move { var, .. } | E::Copy { var, .. } | E::BorrowLocal(_, var) => Some(*var),
            E::Dereference(inner) | E::Borrow(_, inner, _, _) => self.extract_var(inner),
            _ => None,
        }
    }

    /// Mark any unvalidated prices in the guard condition as validated
    /// This handles patterns like: assert!(check_price_is_fresh(&price, max_age), E_STALE)
    fn mark_validated_from_guard(&self, state: &mut PriceState, cond: &Exp) {
        use UnannotatedExp_ as E;
        match &cond.exp.value {
            // Function call in condition - check if it's a validation function
            E::ModuleCall(call) => {
                let func_sym = call.name.value();
                let func_name = func_sym.as_str();

                let is_validation = FRESHNESS_VALIDATION_CALLS
                    .iter()
                    .any(|f| func_name.contains(f));

                if is_validation {
                    // Mark any price variables passed to this function as validated
                    for arg in &call.arguments {
                        if let Some(var) = self.extract_var(arg) {
                            if let Some(LocalState::Available(avail_loc, val)) =
                                state.locals.get(&var)
                            {
                                if matches!(val, PriceValidationValue::Unvalidated(_)) {
                                    state.locals.insert(
                                        var,
                                        LocalState::Available(
                                            *avail_loc,
                                            PriceValidationValue::Validated(cond.exp.loc),
                                        ),
                                    );
                                }
                            }
                        }
                    }
                }
            }
            // Recursively check inside unary/binary expressions
            E::UnaryExp(_, inner) => {
                self.mark_validated_from_guard(state, inner);
            }
            E::BinopExp(lhs, _, rhs) => {
                self.mark_validated_from_guard(state, lhs);
                self.mark_validated_from_guard(state, rhs);
            }
            _ => {}
        }
    }
}

impl SimpleDomain for PriceState {
    type Value = PriceValidationValue;

    fn new(_context: &CFGContext, locals: BTreeMap<Var, LocalState<Self::Value>>) -> Self {
        PriceState { locals }
    }

    fn locals(&self) -> &BTreeMap<Var, LocalState<Self::Value>> {
        &self.locals
    }

    fn locals_mut(&mut self) -> &mut BTreeMap<Var, LocalState<Self::Value>> {
        &mut self.locals
    }

    fn join_value(v1: &Self::Value, v2: &Self::Value) -> Self::Value {
        use PriceValidationValue::*;
        match (v1, v2) {
            // Validated wins (optimistic - if any path validates, we're ok)
            (Validated(loc), _) | (_, Validated(loc)) => Validated(*loc),
            // Unvalidated is concerning
            (Unvalidated(loc), _) | (_, Unvalidated(loc)) => Unvalidated(*loc),
            // Not tracked
            (NotTracked, NotTracked) => NotTracked,
        }
    }

    fn join_impl(&mut self, other: &Self, _result: &mut JoinResult) {
        for (var, other_state) in &other.locals {
            if let Some(self_state) = self.locals.get_mut(var) {
                if let (LocalState::Available(_, v1), LocalState::Available(_, v2)) =
                    (self_state, other_state)
                {
                    *v1 = Self::join_value(v1, v2);
                }
            }
        }
    }
}

impl SimpleExecutionContext for PriceExecutionContext {
    fn add_diag(&mut self, d: CompilerDiagnostic) {
        self.diags.add(d);
    }
}

// ============================================================================
// Public API
// ============================================================================

pub const ABSINT_CUSTOM_DIAG_CODE_MAP: &[(u8, &LintDescriptor)] = &[
    (1, &PHANTOM_CAPABILITY),        // UNUSED_CAP_V2_DIAG
    (2, &UNCHECKED_DIVISION_V2),     // UNCHECKED_DIV_V2_DIAG
    (3, &PHANTOM_CAPABILITY),        // UNVALIDATED_CAP_V2_DIAG
    (4, &DESTROY_ZERO_UNCHECKED_V2), // DESTROY_ZERO_UNCHECKED_V2_DIAG
    (5, &FRESH_ADDRESS_REUSE_V2),    // FRESH_ADDRESS_REUSE_V2_DIAG
    // (6, &TAINTED_TRANSFER_RECIPIENT) - REMOVED: 100% FP rate
    (7, &CAPABILITY_ESCAPE),     // CAPABILITY_ESCAPE_DIAG
    (8, &STALE_ORACLE_PRICE_V3), // STALE_ORACLE_PRICE_V3_DIAG
];

pub fn descriptor_for_diag_code(code: u8) -> Option<&'static LintDescriptor> {
    ABSINT_CUSTOM_DIAG_CODE_MAP
        .iter()
        .find_map(|(c, d)| (*c == code).then_some(*d))
}

static DESCRIPTORS: &[&LintDescriptor] = &[
    &PHANTOM_CAPABILITY,
    &UNCHECKED_DIVISION_V2,
    &DESTROY_ZERO_UNCHECKED_V2,
    &FRESH_ADDRESS_REUSE_V2,
    // TAINTED_TRANSFER_RECIPIENT - REMOVED: 100% FP rate
    &CAPABILITY_ESCAPE,
    &STALE_ORACLE_PRICE_V3,
];

/// Return all Phase II lint descriptors
pub fn descriptors() -> &'static [&'static LintDescriptor] {
    DESCRIPTORS
}

/// Look up a Phase II lint descriptor by name
pub fn find_descriptor(name: &str) -> Option<&'static LintDescriptor> {
    descriptors().iter().copied().find(|d| d.name == name)
}

/// Create Abstract Interpreter visitors for all Phase II lints.
///
/// Phase II lints are gated by `--preview` / `--experimental`:
/// - Preview includes CFG-aware lints that aim for low false positives.
/// - Experimental includes research lints that may be noisier.
pub fn create_visitors(
    preview: bool,
    experimental: bool,
) -> Vec<Box<dyn AbstractInterpreterVisitor>> {
    if !preview && !experimental {
        return Vec::new();
    }

    let mut visitors: Vec<Box<dyn AbstractInterpreterVisitor>> = Vec::new();

    if preview {
        visitors.push(Box::new(UncheckedDivisionVerifier) as Box<dyn AbstractInterpreterVisitor>);
        visitors.push(Box::new(DestroyZeroVerifier) as Box<dyn AbstractInterpreterVisitor>);
        visitors.push(Box::new(FreshAddressReuseVerifier) as Box<dyn AbstractInterpreterVisitor>);
        // TaintedTransferRecipientVerifier removed - 100% FP rate
        visitors.push(Box::new(StaleOraclePriceVerifier) as Box<dyn AbstractInterpreterVisitor>);
    }

    if experimental {
        visitors.push(Box::new(UnusedCapabilityVerifier) as Box<dyn AbstractInterpreterVisitor>);
        visitors.push(Box::new(CapabilityEscapeVerifier) as Box<dyn AbstractInterpreterVisitor>);
    }

    visitors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cap_value_lattice_ordering() {
        use CapValue::*;
        let loc = Loc::invalid();

        // Test ordering: Unused < AccessedNotValidated < PendingValidation < Validated
        assert!(Unused < AccessedNotValidated(loc));
        assert!(AccessedNotValidated(loc) < PendingValidation(loc));
        assert!(PendingValidation(loc) < Validated(loc));
    }

    #[test]
    fn test_cap_value_join() {
        use CapValue::*;
        let loc = Loc::invalid();

        // Validated wins (optimistic)
        assert!(matches!(
            CapState::join_value(&Validated(loc), &Unused),
            Validated(_)
        ));
        assert!(matches!(
            CapState::join_value(&Unused, &Validated(loc)),
            Validated(_)
        ));
        assert!(matches!(
            CapState::join_value(&Validated(loc), &AccessedNotValidated(loc)),
            Validated(_)
        ));

        // PendingValidation next
        assert!(matches!(
            CapState::join_value(&PendingValidation(loc), &Unused),
            PendingValidation(_)
        ));
        assert!(matches!(
            CapState::join_value(&PendingValidation(loc), &AccessedNotValidated(loc)),
            PendingValidation(_)
        ));

        // AccessedNotValidated next
        assert!(matches!(
            CapState::join_value(&AccessedNotValidated(loc), &Unused),
            AccessedNotValidated(_)
        ));

        // Both unused
        assert!(matches!(CapState::join_value(&Unused, &Unused), Unused));
    }

    #[test]
    fn test_divisor_value_join() {
        use DivisorValue::*;

        assert_eq!(DivState::join_value(&Validated, &Validated), Validated);
        assert_eq!(DivState::join_value(&Constant, &Constant), Constant);
        assert_eq!(DivState::join_value(&Validated, &Unknown), Unknown);
        assert_eq!(DivState::join_value(&Constant, &Validated), Validated);
    }
}
