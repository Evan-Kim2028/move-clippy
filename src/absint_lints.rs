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
// 5. Tainted Transfer Recipient (CFG-aware taint analysis)
// ============================================================================

const TAINTED_TRANSFER_RECIPIENT_DIAG: DiagnosticInfo = custom(
    LINT_WARNING_PREFIX,
    Severity::Warning,
    CLIPPY_CATEGORY,
    6, // tainted_transfer_recipient
    "untrusted address used as transfer recipient",
);

pub static TAINTED_TRANSFER_RECIPIENT: LintDescriptor = LintDescriptor {
    name: "tainted_transfer_recipient",
    category: LintCategory::Security,
    description: "Entry function address parameter flows to transfer recipient without validation (type-based CFG-aware, requires --mode full --preview)",
    group: RuleGroup::Preview,
    fix: FixDescriptor::none(),
    analysis: AnalysisKind::TypeBasedCFG,
    gap: Some(TypeSystemGap::ValueFlow),
};

pub struct TaintedTransferRecipientVerifier;

pub struct TaintedTransferRecipientVerifierAI<'a> {
    info: &'a TypingProgramInfo,
    context: &'a CFGContext<'a>,
    /// Entry address params that are taint sources
    tainted_params: Vec<(Var, Loc)>,
    /// Labels whose blocks immediately exit (abort/return)
    exit_blocks: BTreeSet<Label>,
}

/// Taint source classification
/// Classification of taint sources for security analysis
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TaintSource {
    /// Entry function address parameter - attacker can provide arbitrary address
    EntryAddressParam { var: Var, loc: Loc },
    /// Entry function u64/u128 parameter - attacker can provide arbitrary amounts
    EntryAmountParam { var: Var, loc: Loc },
    /// Oracle price data - external data source that could be manipulated
    OraclePrice { call_loc: Loc },
    /// Shared object field read - data from shared objects can be front-run/manipulated
    SharedObjectField { call_loc: Loc },
}

/// Classification of dangerous sinks that should be protected
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaintSink {
    /// Transfer recipient - sending assets to attacker-controlled address
    TransferRecipient,
    /// Coin split amount - extracting attacker-controlled amounts
    CoinSplitAmount,
    /// Dynamic field key - attacker-controlled storage access
    DynamicFieldKey,
}

/// Abstract value for taint tracking
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum TaintValue {
    #[default]
    Untainted,
    /// Value is tainted from an untrusted source
    Tainted { source: TaintSource, loc: Loc },
    /// Value was tainted but has been validated through a guard
    Validated {
        source: TaintSource,
        validation_loc: Loc,
    },
}

/// Abstract state for taint analysis
#[derive(Clone, Debug)]
pub struct TaintState {
    locals: BTreeMap<Var, LocalState<TaintValue>>,
    /// Maps temporary variables to the tainted variables they validate.
    /// When we see `$tmp = (tainted_var == something)`, we record $tmp -> tainted_var
    /// so that when JumpIf uses $tmp, we can mark tainted_var as validated.
    validation_temps: BTreeMap<Var, Vec<Var>>,
}

/// Execution context for taint analysis
pub struct TaintExecutionContext {
    diags: CompilerDiagnostics,
}

impl SimpleAbsIntConstructor for TaintedTransferRecipientVerifier {
    type AI<'a> = TaintedTransferRecipientVerifierAI<'a>;

    fn new<'a>(
        context: &'a CFGContext<'a>,
        cfg: &ImmForwardCFG,
        init_state: &mut TaintState,
    ) -> Option<Self::AI<'a>> {
        // Skip test functions
        if context.attributes.is_test_or_test_only() {
            return None;
        }

        // Only analyze entry functions - they receive untrusted input
        if context.entry.is_none() {
            return None;
        }

        // Find tainted parameters (addresses and amounts)
        let mut tainted_params: Vec<(Var, Loc, TaintSource)> = Vec::new();

        for (_, var, ty) in &context.signature.parameters {
            // Skip underscore-prefixed params (intentional escape hatch)
            if var.starts_with_underscore() {
                continue;
            }

            let loc = var.0.loc;

            // Address parameters - can be attacker-controlled recipients
            if is_address_type(ty) {
                tainted_params.push((*var, loc, TaintSource::EntryAddressParam { var: *var, loc }));
            }
            // Amount parameters (u64, u128) - can be attacker-controlled amounts
            else if is_amount_type(ty) {
                tainted_params.push((*var, loc, TaintSource::EntryAmountParam { var: *var, loc }));
            }
        }

        if tainted_params.is_empty() {
            return None; // No taint sources
        }

        // Initialize tainted params in initial state
        for (var, loc, source) in &tainted_params {
            if let Some(local_state) = init_state.locals.get_mut(var) {
                if let LocalState::Available(avail_loc, _) = local_state {
                    *local_state = LocalState::Available(
                        *avail_loc,
                        TaintValue::Tainted {
                            source: source.clone(),
                            loc: *loc,
                        },
                    );
                }
            }
        }

        // Convert to simple (Var, Loc) for struct storage
        let tainted_params: Vec<(Var, Loc)> = tainted_params
            .into_iter()
            .map(|(var, loc, _)| (var, loc))
            .collect();

        // Precompute exit blocks for guard detection
        let mut exit_blocks = BTreeSet::new();
        for lbl in cfg.block_labels() {
            if is_immediate_exit_block(cfg, lbl) {
                exit_blocks.insert(lbl);
            }
        }

        Some(TaintedTransferRecipientVerifierAI {
            info: context.info,
            context,
            tainted_params,
            exit_blocks,
        })
    }
}

impl SimpleAbsInt for TaintedTransferRecipientVerifierAI<'_> {
    type State = TaintState;
    type ExecutionContext = TaintExecutionContext;

    fn finish(
        &mut self,
        _final_states: BTreeMap<Label, Self::State>,
        diags: CompilerDiagnostics,
    ) -> CompilerDiagnostics {
        diags
    }

    fn start_command(&self, _pre: &mut Self::State) -> Self::ExecutionContext {
        TaintExecutionContext {
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
                // Visit condition first
                self.exp(context, state, cond);

                // Check if one branch is an exit (abort) - this is a guard pattern
                let true_is_abort = self.exit_blocks.contains(if_true);
                let false_is_abort = self.exit_blocks.contains(if_false);

                // Trigger validation if at least one branch is an abort (guard pattern)
                // Note: Both can be abort in some CFG structures from assert! compilation
                if true_is_abort || false_is_abort {
                    // This is a guard pattern - mark any tainted variables in condition as validated
                    // Also check if the condition ITSELF is a comparison with tainted vars
                    self.mark_validated_from_guard(state, cond, cmd.loc);
                    // Also try to validate directly from condition if it contains comparison
                    self.track_validation_in_exp(state, cond, cmd.loc);
                }

                true // Handled
            }
            C::Assign(_case, lvalues, rhs) => {
                // First, check if this is a comparison involving a tainted variable
                // If so, record the LHS as a "validation temp" for that tainted var
                self.track_validation_comparison(state, lvalues, rhs);

                // Collect taint from RHS expression
                let rhs_taints = self.exp(context, state, rhs);
                let rhs_taint = rhs_taints
                    .into_iter()
                    .find(|t| matches!(t, TaintValue::Tainted { .. }));

                // Propagate taint to LHS variables
                for lvalue in lvalues {
                    if let LValue_::Var { var, .. } = &lvalue.value {
                        if let Some(ref taint) = rhs_taint {
                            if let TaintValue::Tainted { source, .. } = taint {
                                if let Some(local_state) = state.locals.get_mut(var) {
                                    if let LocalState::Available(avail_loc, _) = local_state {
                                        *local_state = LocalState::Available(
                                            *avail_loc,
                                            TaintValue::Tainted {
                                                source: source.clone(),
                                                loc: lvalue.loc,
                                            },
                                        );
                                    }
                                }
                            }
                        }
                    }
                }

                false // Let default handling continue
            }
            _ => false,
        }
    }

    fn exp_custom(
        &self,
        context: &mut Self::ExecutionContext,
        state: &mut Self::State,
        e: &Exp,
    ) -> Option<Vec<TaintValue>> {
        use UnannotatedExp_ as E;

        match &e.exp.value {
            // Variable access - propagate taint
            E::Move { var, .. } | E::Copy { var, .. } | E::BorrowLocal(_, var) => {
                if let Some(LocalState::Available(_, taint)) = state.locals.get(var) {
                    return Some(vec![taint.clone()]);
                }
            }
            // Detect assert! calls and mark tainted variables in condition as validated
            E::ModuleCall(call) => {
                if self.is_assert_call(call) {
                    if let Some(cond) = call.arguments.first() {
                        // Track validation comparison in the assert condition
                        self.track_validation_in_exp(state, cond, e.exp.loc);
                    }
                }
            }
            _ => {}
        }

        None // Use default traversal
    }

    fn call_custom(
        &self,
        context: &mut Self::ExecutionContext,
        state: &mut Self::State,
        loc: &Loc,
        _return_ty: &Type,
        call: &ModuleCall,
        args: Vec<TaintValue>,
    ) -> Option<Vec<TaintValue>> {
        // Check for tainted values reaching dangerous sinks
        if let Some((sink, arg_idx)) = self.get_sink_info(call) {
            if let Some(TaintValue::Tainted { source, .. }) = args.get(arg_idx) {
                self.report_taint_sink_violation(context, loc, call, sink, source);
            }
        }

        None
    }
}

impl TaintedTransferRecipientVerifierAI<'_> {
    /// Determine if a call is a dangerous sink and which argument index is sensitive
    fn get_sink_info(&self, call: &ModuleCall) -> Option<(TaintSink, usize)> {
        // transfer::public_transfer(obj, recipient) -> recipient is index 1
        // transfer::transfer(obj, recipient) -> recipient is index 1
        if call.is(&SUI_ADDR, "transfer", "public_transfer")
            || call.is(&SUI_ADDR, "transfer", "transfer")
        {
            return Some((TaintSink::TransferRecipient, 1));
        }

        // coin::split(coin, amount) -> amount is index 1
        if call.is(&SUI_ADDR, "coin", "split") {
            return Some((TaintSink::CoinSplitAmount, 1));
        }

        // dynamic_field::add(obj, key, value) -> key is index 1
        // dynamic_field::borrow(obj, key) -> key is index 1
        // dynamic_field::borrow_mut(obj, key) -> key is index 1
        // dynamic_field::remove(obj, key) -> key is index 1
        if call.is(&SUI_ADDR, "dynamic_field", "add")
            || call.is(&SUI_ADDR, "dynamic_field", "borrow")
            || call.is(&SUI_ADDR, "dynamic_field", "borrow_mut")
            || call.is(&SUI_ADDR, "dynamic_field", "remove")
            || call.is(&SUI_ADDR, "dynamic_object_field", "add")
            || call.is(&SUI_ADDR, "dynamic_object_field", "borrow")
            || call.is(&SUI_ADDR, "dynamic_object_field", "borrow_mut")
            || call.is(&SUI_ADDR, "dynamic_object_field", "remove")
        {
            return Some((TaintSink::DynamicFieldKey, 1));
        }

        None
    }

    /// Report a taint sink violation with appropriate messaging
    fn report_taint_sink_violation(
        &self,
        context: &mut TaintExecutionContext,
        loc: &Loc,
        call: &ModuleCall,
        sink: TaintSink,
        source: &TaintSource,
    ) {
        let (source_name, source_loc, source_note) = match source {
            TaintSource::EntryAddressParam { var, loc } => (
                var.value().as_str().to_owned(),
                *loc,
                "Entry function parameters can be controlled by attackers",
            ),
            TaintSource::EntryAmountParam { var, loc } => (
                var.value().as_str().to_owned(),
                *loc,
                "Entry function amount parameters can be controlled by attackers",
            ),
            TaintSource::OraclePrice { call_loc } => (
                "oracle_price".to_owned(),
                *call_loc,
                "Oracle prices can be manipulated through flash loans or price oracle attacks",
            ),
            TaintSource::SharedObjectField { call_loc } => (
                "shared_field".to_owned(),
                *call_loc,
                "Shared object fields can be modified by concurrent transactions",
            ),
        };

        let (msg, help) = match sink {
            TaintSink::TransferRecipient => (
                format!(
                    "Untrusted address parameter `{}` flows to transfer recipient without validation",
                    source_name
                ),
                "Add validation: `assert!(recipient == ctx.sender(), E_UNAUTHORIZED)` or use `_recipient` to suppress",
            ),
            TaintSink::CoinSplitAmount => (
                format!(
                    "Untrusted amount `{}` flows to coin::split without validation",
                    source_name
                ),
                "Add bounds checking: `assert!(amount <= max_allowed, E_AMOUNT_TOO_LARGE)`",
            ),
            TaintSink::DynamicFieldKey => (
                format!(
                    "Untrusted key `{}` flows to dynamic field operation without validation",
                    source_name
                ),
                "Validate the key against expected values or use a whitelist",
            ),
        };

        context.add_diag(diag!(
            TAINTED_TRANSFER_RECIPIENT_DIAG,
            (*loc, msg),
            (source_loc, source_note),
            (call.name.0.loc, help),
        ));
    }

    fn is_assert_call(&self, call: &ModuleCall) -> bool {
        let func_sym = call.name.value();
        let func_name = func_sym.as_str();
        func_name == "assert" || func_name.contains("assert")
    }

    /// Track validation in an expression (e.g., assert! condition).
    /// Recursively finds comparisons involving tainted vars and marks them validated.
    fn track_validation_in_exp(&self, state: &mut TaintState, exp: &Exp, validation_loc: Loc) {
        use UnannotatedExp_ as E;

        match &exp.exp.value {
            // Direct comparison - mark tainted vars as validated
            E::BinopExp(lhs, op, rhs) => {
                if matches!(op.value, BinOp_::Eq | BinOp_::Neq) {
                    // Mark any tainted vars in this comparison as validated
                    let tainted_vars = self.find_tainted_vars_deep(state, exp);
                    for var in tainted_vars {
                        if let Some(local_state) = state.locals.get_mut(&var) {
                            if let LocalState::Available(
                                avail_loc,
                                TaintValue::Tainted { source, .. },
                            ) = local_state
                            {
                                *local_state = LocalState::Available(
                                    *avail_loc,
                                    TaintValue::Validated {
                                        source: source.clone(),
                                        validation_loc,
                                    },
                                );
                            }
                        }
                    }
                } else if matches!(op.value, BinOp_::And | BinOp_::Or) {
                    // Recurse into && and ||
                    self.track_validation_in_exp(state, lhs, validation_loc);
                    self.track_validation_in_exp(state, rhs, validation_loc);
                }
            }
            E::UnaryExp(_, inner) => {
                self.track_validation_in_exp(state, inner, validation_loc);
            }
            // If the condition is a variable, check if it's a validation temp
            E::Move { var, .. } | E::Copy { var, .. } => {
                if let Some(validated_vars) = state.validation_temps.get(var).cloned() {
                    for tainted_var in validated_vars {
                        if let Some(local_state) = state.locals.get_mut(&tainted_var) {
                            if let LocalState::Available(
                                avail_loc,
                                TaintValue::Tainted { source, .. },
                            ) = local_state
                            {
                                *local_state = LocalState::Available(
                                    *avail_loc,
                                    TaintValue::Validated {
                                        source: source.clone(),
                                        validation_loc,
                                    },
                                );
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    /// Track when a comparison result is assigned to a temporary.
    /// e.g., `$tmp = (recipient == ctx.sender())` -> record $tmp validates recipient
    fn track_validation_comparison(&self, state: &mut TaintState, lvalues: &[LValue], rhs: &Exp) {
        use UnannotatedExp_ as E;

        // Recursively find tainted vars in the RHS expression and track them
        let tainted_vars = self.find_tainted_vars_deep(state, rhs);

        // Check if RHS contains a comparison (Eq, Neq) - for validation temps
        if self.expression_contains_comparison(rhs) && !tainted_vars.is_empty() {
            // Record that the LHS variable(s) validate these tainted vars
            for lvalue in lvalues {
                if let LValue_::Var { var, .. } = &lvalue.value {
                    state.validation_temps.insert(*var, tainted_vars.clone());
                }
            }
        }
    }

    /// Check if an expression contains a comparison operation
    fn expression_contains_comparison(&self, exp: &Exp) -> bool {
        use UnannotatedExp_ as E;
        match &exp.exp.value {
            E::BinopExp(lhs, op, rhs) => {
                if matches!(op.value, BinOp_::Eq | BinOp_::Neq) {
                    return true;
                }
                self.expression_contains_comparison(lhs) || self.expression_contains_comparison(rhs)
            }
            E::UnaryExp(_, inner)
            | E::Cast(inner, _)
            | E::Freeze(inner)
            | E::Dereference(inner) => self.expression_contains_comparison(inner),
            _ => false,
        }
    }

    /// Recursively find all tainted variables in an expression
    fn find_tainted_vars_deep(&self, state: &TaintState, exp: &Exp) -> Vec<Var> {
        use UnannotatedExp_ as E;
        let mut result = Vec::new();

        match &exp.exp.value {
            E::Move { var, .. } | E::Copy { var, .. } | E::BorrowLocal(_, var) => {
                if let Some(LocalState::Available(_, TaintValue::Tainted { .. })) =
                    state.locals.get(var)
                {
                    result.push(*var);
                }
            }
            E::BinopExp(lhs, _, rhs) => {
                result.extend(self.find_tainted_vars_deep(state, lhs));
                result.extend(self.find_tainted_vars_deep(state, rhs));
            }
            E::UnaryExp(_, inner)
            | E::Cast(inner, _)
            | E::Freeze(inner)
            | E::Dereference(inner) => {
                result.extend(self.find_tainted_vars_deep(state, inner));
            }
            E::Borrow(_, inner, _, _) => {
                result.extend(self.find_tainted_vars_deep(state, inner));
            }
            E::ModuleCall(call) => {
                for arg in &call.arguments {
                    result.extend(self.find_tainted_vars_deep(state, arg));
                }
            }
            E::Multiple(exps) => {
                for e in exps {
                    result.extend(self.find_tainted_vars_deep(state, e));
                }
            }
            E::Vector(_, _, _, exps) => {
                for e in exps {
                    result.extend(self.find_tainted_vars_deep(state, e));
                }
            }
            _ => {}
        }

        result
    }

    /// Collect variables that are currently tainted from an expression
    fn collect_tainted_vars_from_exp(&self, state: &TaintState, exp: &Exp) -> Vec<Var> {
        use UnannotatedExp_ as E;
        let mut result = Vec::new();

        match &exp.exp.value {
            E::Move { var, .. } | E::Copy { var, .. } | E::BorrowLocal(_, var) => {
                if let Some(LocalState::Available(_, TaintValue::Tainted { .. })) =
                    state.locals.get(var)
                {
                    result.push(*var);
                }
            }
            E::BinopExp(lhs, _, rhs) => {
                result.extend(self.collect_tainted_vars_from_exp(state, lhs));
                result.extend(self.collect_tainted_vars_from_exp(state, rhs));
            }
            E::UnaryExp(_, inner) | E::Cast(inner, _) => {
                result.extend(self.collect_tainted_vars_from_exp(state, inner));
            }
            E::Borrow(_, inner, _, _) => {
                result.extend(self.collect_tainted_vars_from_exp(state, inner));
            }
            _ => {}
        }

        result
    }

    /// Mark tainted variables as validated when we see a guard pattern.
    /// Handles both direct variable references and validation temporaries.
    fn mark_validated_from_guard(&self, state: &mut TaintState, cond: &Exp, validation_loc: Loc) {
        use UnannotatedExp_ as E;

        // Collect vars directly in the condition
        let direct_vars = self.collect_vars_from_exp(cond);

        // Also check if the condition references a validation temp
        let mut vars_to_validate: Vec<Var> = Vec::new();

        for var in &direct_vars {
            // Check if this var is a validation temp
            if let Some(validated_vars) = state.validation_temps.get(var) {
                vars_to_validate.extend(validated_vars.iter().cloned());
            }
            // Also add the var itself if it's directly tainted
            vars_to_validate.push(*var);
        }

        // Mark all collected vars as validated
        for var in vars_to_validate {
            if let Some(local_state) = state.locals.get_mut(&var) {
                if let LocalState::Available(avail_loc, TaintValue::Tainted { source, .. }) =
                    local_state
                {
                    *local_state = LocalState::Available(
                        *avail_loc,
                        TaintValue::Validated {
                            source: source.clone(),
                            validation_loc,
                        },
                    );
                }
            }
        }
    }

    fn collect_vars_from_exp(&self, exp: &Exp) -> Vec<Var> {
        use UnannotatedExp_ as E;
        let mut vars = Vec::new();

        match &exp.exp.value {
            E::Move { var, .. } | E::Copy { var, .. } | E::BorrowLocal(_, var) => {
                vars.push(*var);
            }
            E::BinopExp(lhs, _, rhs) => {
                vars.extend(self.collect_vars_from_exp(lhs));
                vars.extend(self.collect_vars_from_exp(rhs));
            }
            E::UnaryExp(_, inner) => {
                vars.extend(self.collect_vars_from_exp(inner));
            }
            E::Cast(inner, _) => {
                vars.extend(self.collect_vars_from_exp(inner));
            }
            E::Borrow(_, inner, _, _) => {
                vars.extend(self.collect_vars_from_exp(inner));
            }
            E::Multiple(exps) => {
                for e in exps {
                    vars.extend(self.collect_vars_from_exp(e));
                }
            }
            _ => {}
        }

        vars
    }
}

/// Check if a type is `address`
fn is_address_type(ty: &SingleType) -> bool {
    match &ty.value {
        SingleType_::Base(bt) => is_address_base_type(&bt.value),
        SingleType_::Ref(_, bt) => is_address_base_type(&bt.value),
    }
}

fn is_address_base_type(bt: &BaseType_) -> bool {
    matches!(
        bt,
        BaseType_::Apply(_, type_name, _)
            if matches!(
                &type_name.value,
                TypeName_::Builtin(builtin) if matches!(builtin.value, BuiltinTypeName_::Address)
            )
    )
}

/// Check if a type is an amount type (u64, u128) that could be attacker-controlled
fn is_amount_type(ty: &SingleType) -> bool {
    match &ty.value {
        SingleType_::Base(bt) => is_amount_base_type(&bt.value),
        SingleType_::Ref(_, bt) => is_amount_base_type(&bt.value),
    }
}

fn is_amount_base_type(bt: &BaseType_) -> bool {
    matches!(
        bt,
        BaseType_::Apply(_, type_name, _)
            if matches!(
                &type_name.value,
                TypeName_::Builtin(builtin)
                    if matches!(builtin.value, BuiltinTypeName_::U64 | BuiltinTypeName_::U128)
            )
    )
}

impl SimpleDomain for TaintState {
    type Value = TaintValue;

    fn new(_context: &CFGContext, locals: BTreeMap<Var, LocalState<Self::Value>>) -> Self {
        TaintState {
            locals,
            validation_temps: BTreeMap::new(),
        }
    }

    fn locals_mut(&mut self) -> &mut BTreeMap<Var, LocalState<Self::Value>> {
        &mut self.locals
    }

    fn locals(&self) -> &BTreeMap<Var, LocalState<Self::Value>> {
        &self.locals
    }

    fn join_value(v1: &Self::Value, v2: &Self::Value) -> Self::Value {
        use TaintValue::*;
        // Pessimistic: if ANY path is tainted (not validated), result is tainted
        match (v1, v2) {
            // Both validated = validated
            (
                Validated {
                    source,
                    validation_loc,
                },
                Validated { .. },
            ) => Validated {
                source: source.clone(),
                validation_loc: *validation_loc,
            },
            // Tainted wins over validated (pessimistic - all paths must validate)
            (Tainted { source, loc }, _) | (_, Tainted { source, loc }) => Tainted {
                source: source.clone(),
                loc: *loc,
            },
            // Validated over untainted
            (
                Validated {
                    source,
                    validation_loc,
                },
                Untainted,
            )
            | (
                Untainted,
                Validated {
                    source,
                    validation_loc,
                },
            ) => Validated {
                source: source.clone(),
                validation_loc: *validation_loc,
            },
            // Both untainted
            (Untainted, Untainted) => Untainted,
        }
    }

    fn join_impl(&mut self, other: &Self, _result: &mut JoinResult) {
        // Merge validation_temps from other
        for (var, validated_vars) in &other.validation_temps {
            self.validation_temps
                .entry(*var)
                .or_insert_with(Vec::new)
                .extend(validated_vars.iter().cloned());
        }
    }
}

impl SimpleExecutionContext for TaintExecutionContext {
    fn add_diag(&mut self, d: CompilerDiagnostic) {
        self.diags.add(d);
    }
}

// ============================================================================
// Public API
// ============================================================================

pub const ABSINT_CUSTOM_DIAG_CODE_MAP: &[(u8, &LintDescriptor)] = &[
    (1, &PHANTOM_CAPABILITY),         // UNUSED_CAP_V2_DIAG
    (2, &UNCHECKED_DIVISION_V2),      // UNCHECKED_DIV_V2_DIAG
    (3, &PHANTOM_CAPABILITY),         // UNVALIDATED_CAP_V2_DIAG
    (4, &DESTROY_ZERO_UNCHECKED_V2),  // DESTROY_ZERO_UNCHECKED_V2_DIAG
    (5, &FRESH_ADDRESS_REUSE_V2),     // FRESH_ADDRESS_REUSE_V2_DIAG
    (6, &TAINTED_TRANSFER_RECIPIENT), // TAINTED_TRANSFER_RECIPIENT_DIAG
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
    &TAINTED_TRANSFER_RECIPIENT,
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
        visitors.push(Box::new(TaintedTransferRecipientVerifier) as Box<dyn AbstractInterpreterVisitor>);
    }

    if experimental {
        visitors.push(Box::new(UnusedCapabilityVerifier) as Box<dyn AbstractInterpreterVisitor>);
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
