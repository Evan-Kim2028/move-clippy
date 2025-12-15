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

use crate::diagnostics::Diagnostic;
use crate::error::ClippyResult;
use crate::lint::{
    AnalysisKind, FixDescriptor, LintCategory, LintDescriptor, LintSettings, RuleGroup,
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

// ============================================================================
// Phase II Lint Descriptors (type-based with CFG analysis)
// ============================================================================

pub static PHANTOM_CAPABILITY: LintDescriptor = LintDescriptor {
    name: "phantom_capability",
    category: LintCategory::Security,
    description: "Capability parameter unused or not validated - may be phantom security (type-based CFG-aware, requires --mode full --preview)",
    group: RuleGroup::Preview,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBasedCFG,
};

pub static UNCHECKED_DIVISION_V2: LintDescriptor = LintDescriptor {
    name: "unchecked_division_v2",
    category: LintCategory::Security,
    description: "Division without zero-check (type-based CFG-aware, requires --mode full --preview)",
    group: RuleGroup::Preview,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBasedCFG,
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
    /// Tracks whether each capability was validated through a guard
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
                    move_compiler::naming::ast::TypeName_::ModuleType(m, n) => {
                        // Check for Balance<T> type
                        m.value.is(&SUI_ADDR, "balance") && n.0.value.as_str() == "Balance"
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

    fn is_root_source_loc(&self, context: &CFGContext, loc: &Loc) -> bool {
        let _ = loc;
        !context.is_dependency && context.is_root_package
    }

    fn debug_provenance_if_enabled(&self, context: &CFGContext, loc: &Loc) {
        if std::env::var("MOVE_CLIPPY_DEBUG_PHASE2_PROVENANCE").as_deref() != Ok("1") {
            return;
        }

        let file_hash = loc.file_hash();
        let file_path = context.files.file_path(&file_hash).to_string_lossy();

        eprintln!(
            "[move-clippy][phase2][unchecked_division_v2] file={} package={:?} is_dependency={} is_root_package={} target_kind={:?}",
            file_path,
            context.package,
            context.is_dependency,
            context.is_root_package,
            context.target_kind
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
        let name_symbol = call.name.value();
        let func_name = name_symbol.as_str();
        func_name == "assert" || func_name.contains("assert")
    }

    fn track_nonzero_guard(&self, state: &mut DivState, condition: &Exp, cond_value: bool) {
        if let Some((var, nonzero_when_true)) = self.extract_nonzero_guard(condition) {
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

    fn extract_nonzero_guard(&self, condition: &Exp) -> Option<(Var, bool)> {
        match &condition.exp.value {
            UnannotatedExp_::UnaryExp(unop, inner)
                if matches!(unop.value, move_compiler::parser::ast::UnaryOp_::Not) =>
            {
                self.extract_nonzero_guard(inner)
                    .map(|(v, nonzero_when_true)| (v, !nonzero_when_true))
            }
            UnannotatedExp_::Cast(inner, _) => self.extract_nonzero_guard(inner),
            UnannotatedExp_::BinopExp(lhs, op, rhs) => {
                // Handle `a && b` by extracting from either side (best-effort).
                if matches!(op.value, BinOp_::And) {
                    return self
                        .extract_nonzero_guard(lhs)
                        .or_else(|| self.extract_nonzero_guard(rhs));
                }

                // Prefer patterns where one side is a literal, and the other is a local var.
                let (var_side, lit_side, var_on_lhs) =
                    if matches!(&rhs.exp.value, UnannotatedExp_::Value(_)) {
                        (lhs, rhs, true)
                    } else if matches!(&lhs.exp.value, UnannotatedExp_::Value(_)) {
                        (rhs, lhs, false)
                    } else {
                        return None;
                    };

                let var = self.extract_var(var_side)?;
                let UnannotatedExp_::Value(v) = &lit_side.exp.value else {
                    return None;
                };
                let lit_is_zero = v.value.is_zero();

                // Returns whether the condition being TRUE implies the var is non-zero.
                let nonzero_when_true = match (op.value, lit_is_zero, var_on_lhs) {
                    // var != 0  OR  0 != var
                    (BinOp_::Neq, true, _) => true,
                    // var == 0  OR  0 == var
                    (BinOp_::Eq, true, _) => false,

                    // var > 0
                    (BinOp_::Gt, true, true) => true,
                    // 0 < var
                    (BinOp_::Lt, true, false) => true,

                    // var >= K where K != 0
                    (BinOp_::Ge, false, true) => true,
                    // K <= var where K != 0
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
    fn add_diag(&mut self, diag: CompilerDiagnostic) {
        self.diags.add(diag)
    }
}

// ============================================================================
// Public API
// ============================================================================

static DESCRIPTORS: &[&LintDescriptor] = &[&PHANTOM_CAPABILITY, &UNCHECKED_DIVISION_V2];

/// Return all Phase II lint descriptors
pub fn descriptors() -> &'static [&'static LintDescriptor] {
    DESCRIPTORS
}

/// Look up a Phase II lint descriptor by name
pub fn find_descriptor(name: &str) -> Option<&'static LintDescriptor> {
    descriptors().iter().copied().find(|d| d.name == name)
}

/// Create Abstract Interpreter visitors for all Phase II lints
pub fn create_visitors(preview: bool) -> Vec<Box<dyn AbstractInterpreterVisitor>> {
    if !preview {
        return Vec::new();
    }

    vec![
        Box::new(UnusedCapabilityVerifier) as Box<dyn AbstractInterpreterVisitor>,
        Box::new(UncheckedDivisionVerifier) as Box<dyn AbstractInterpreterVisitor>,
    ]
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

    #[test]
    fn test_hot_potato_value_join() {
        use HotPotatoValue::*;
        let loc = Loc::invalid();

        // Pessimistic: if either path has an unconsumed hot potato, keep it
        assert!(matches!(
            HotPotatoState::join_value(&FreshHotPotato(loc), &NotHotPotato),
            FreshHotPotato(_)
        ));
        assert!(matches!(
            HotPotatoState::join_value(&NotHotPotato, &FreshHotPotato(loc)),
            FreshHotPotato(_)
        ));
        assert!(matches!(
            HotPotatoState::join_value(&FreshHotPotato(loc), &FreshHotPotato(loc)),
            FreshHotPotato(_)
        ));
        assert!(matches!(
            HotPotatoState::join_value(&NotHotPotato, &NotHotPotato),
            NotHotPotato
        ));
    }
}
