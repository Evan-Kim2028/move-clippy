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

#![cfg(feature = "full")]
#![allow(unused)]

use crate::diagnostics::Diagnostic;
use crate::error::ClippyResult;
use crate::lint::{LintCategory, LintDescriptor, LintSettings, RuleGroup, FixDescriptor};
use move_compiler::{
    PreCompiledProgramInfo,
    cfgir::{
        CFGContext,
        absint::JoinResult,
        cfg::ImmForwardCFG,
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
        BaseType, BaseType_, Exp, LValue, LValue_, Label, ModuleCall, SingleType, SingleType_,
        Type, Type_, UnannotatedExp_, Var, Value as HValue, Value_,
    },
    naming::ast::{BuiltinTypeName_, Type as NType, Type_ as NType_},
    parser::ast::{Ability_, DatatypeName},
    shared::{
        Identifier,
        program_info::TypingProgramInfo,
    },
};
use move_ir_types::location::*;
use move_proc_macros::growing_stack;
use std::{collections::BTreeMap, sync::Arc};

// ============================================================================
// Lint Diagnostic Codes
// ============================================================================

const LINT_WARNING_PREFIX: &str = "Lint";
const CLIPPY_CATEGORY: u8 = 200; // Custom category for move-clippy

const UNUSED_CAP_V2_DIAG: DiagnosticInfo = custom(
    LINT_WARNING_PREFIX,
    Severity::Warning,
    CLIPPY_CATEGORY,
    1, // unused_capability_param_v2
    "capability parameter is unused",
);

const UNCHECKED_DIV_V2_DIAG: DiagnosticInfo = custom(
    LINT_WARNING_PREFIX,
    Severity::Warning,
    CLIPPY_CATEGORY,
    2, // unchecked_division_v2
    "division without zero-check validation",
);

const ORACLE_TAINT_DIAG: DiagnosticInfo = custom(
    LINT_WARNING_PREFIX,
    Severity::Warning,
    CLIPPY_CATEGORY,
    3, // oracle_price_taint
    "untrusted oracle price flows to calculation",
);

const RESOURCE_LEAK_DIAG: DiagnosticInfo = custom(
    LINT_WARNING_PREFIX,
    Severity::Warning,
    CLIPPY_CATEGORY,
    4, // resource_leak
    "resource value not consumed on all paths",
);

// ============================================================================
// Phase II Lint Descriptors
// ============================================================================

pub static UNUSED_CAPABILITY_PARAM_V2: LintDescriptor = LintDescriptor {
    name: "unused_capability_param_v2",
    category: LintCategory::Security,
    description: "Capability parameter unused (CFG-aware, requires --mode full)",
    group: RuleGroup::Preview,
    fix: FixDescriptor::none(),
};

pub static UNCHECKED_DIVISION_V2: LintDescriptor = LintDescriptor {
    name: "unchecked_division_v2",
    category: LintCategory::Security,
    description: "Division without zero-check (CFG-aware, requires --mode full)",
    group: RuleGroup::Preview,
    fix: FixDescriptor::none(),
};

pub static ORACLE_PRICE_TAINT: LintDescriptor = LintDescriptor {
    name: "oracle_price_taint",
    category: LintCategory::Security,
    description: "Untrusted oracle price flows to calculation (taint tracking)",
    group: RuleGroup::Preview,
    fix: FixDescriptor::none(),
};

pub static RESOURCE_LEAK: LintDescriptor = LintDescriptor {
    name: "resource_leak",
    category: LintCategory::Security,
    description: "Resource value not consumed on all paths (CFG-aware)",
    group: RuleGroup::Preview,
    fix: FixDescriptor::none(),
};

// ============================================================================
// 1. Unused Capability Parameter (CFG-aware)
// ============================================================================

pub struct UnusedCapabilityVerifier;

pub struct UnusedCapabilityVerifierAI<'a> {
    /// Capability parameters to track
    cap_params: Vec<(Var, Loc)>,
    /// Module typing information
    info: &'a TypingProgramInfo,
}

/// Abstract value: tracks whether a capability has been used
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum CapValue {
    /// Capability never accessed
    #[default]
    Unused,
    /// Capability was accessed/validated
    Used,
}

/// Execution context for capability tracking
pub struct CapExecutionContext {
    diags: CompilerDiagnostics,
}

/// Abstract state: mapping from variables to capability usage
#[derive(Clone, Debug)]
pub struct CapState {
    locals: BTreeMap<Var, LocalState<CapValue>>,
}

impl SimpleAbsIntConstructor for UnusedCapabilityVerifier {
    type AI<'a> = UnusedCapabilityVerifierAI<'a>;

    fn new<'a>(
        context: &'a CFGContext<'a>,
        _cfg: &ImmForwardCFG,
        init_state: &mut CapState,
    ) -> Option<Self::AI<'a>> {
        // Skip test functions
        if context.attributes.is_test_or_test_only() {
            return None;
        }

        // Find capability parameters
        let cap_params: Vec<(Var, Loc)> = context
            .signature
            .parameters
            .iter()
            .filter_map(|(_, var, ty)| {
                if is_capability_param(var, ty) {
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
        for (cap_var, loc) in &cap_params {
            if let Some(LocalState::Available(_, value)) = init_state.locals.get_mut(cap_var) {
                *value = CapValue::Unused;
            }
        }

        Some(UnusedCapabilityVerifierAI {
            cap_params,
            info: context.info,
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

        // Check if any capability is unused in all final states
        for (cap_var, cap_loc) in &self.cap_params {
            let all_unused = final_states.values().all(|state| {
                matches!(
                    state.locals.get(cap_var),
                    Some(LocalState::Available(_, CapValue::Unused)) | Some(LocalState::Unavailable(_, _))
                )
            });

            if all_unused {
                let cap_name = cap_var.value.name.as_str();
                let msg = format!(
                    "Capability parameter `{cap_name}` is never used, indicating missing access control"
                );
                let help = "Capabilities should be validated via assertions like `assert!(cap.pool_id == object::id(pool), E_WRONG_CAP)` or used in access control checks";

                let d = diag!(
                    UNUSED_CAP_V2_DIAG,
                    (*cap_loc, msg),
                    (*cap_loc, help),
                );
                result_diags.add(d);
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

    fn exp_custom(
        &self,
        _context: &mut Self::ExecutionContext,
        state: &mut Self::State,
        e: &Exp,
    ) -> Option<Vec<CapValue>> {
        use UnannotatedExp_ as E;

        // Mark capability as Used when accessed
        match &e.exp.value {
            E::BorrowLocal(_, var) | E::Move { var, .. } | E::Copy { var, .. } => {
                if self.is_tracked_cap(var) {
                    if let Some(local_state) = state.locals.get_mut(var) {
                        if let LocalState::Available(loc, _) = local_state {
                            *local_state = LocalState::Available(*loc, CapValue::Used);
                        }
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
        _f: &ModuleCall,
        args: Vec<CapValue>,
    ) -> Option<Vec<CapValue>> {
        // If any argument is a Used capability, propagate that state
        // This handles capabilities passed to functions
        for arg in &args {
            if *arg == CapValue::Used {
                // Capability is being used
                // Note: We can't easily track which var this came from in the call_custom hook
                // The exp_custom hook handles direct variable usage
            }
        }

        None
    }
}

impl UnusedCapabilityVerifierAI<'_> {
    fn is_tracked_cap(&self, var: &Var) -> bool {
        self.cap_params.iter().any(|(cap, _)| cap.value.id == var.value.id)
    }
}

impl SimpleDomain for CapState {
    type Value = CapValue;

    fn new(_context: &CFGContext, locals: BTreeMap<Var, LocalState<Self::Value>>) -> Self {
        CapState { locals }
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
            (Used, _) | (_, Used) => Used, // Once used, always used
            (Unused, Unused) => Unused,
        }
    }

    fn join_impl(&mut self, _other: &Self, _result: &mut JoinResult) {
        // No additional joining logic needed
    }
}

impl SimpleExecutionContext for CapExecutionContext {
    fn add_diag(&mut self, diag: CompilerDiagnostic) {
        self.diags.add(diag)
    }
}

/// Check if a parameter looks like a capability
fn is_capability_param(var: &Var, ty: &SingleType) -> bool {
    let name = var.value.name.as_str();
    let is_cap_name = name.ends_with("_cap")
        || name.ends_with("Cap")
        || name == "cap"
        || name.starts_with("admin")
        || name.contains("capability");

    // Also check if type has key+store abilities (capability pattern)
    let is_cap_type = matches!(&ty.value, SingleType_::Ref(_, bt)
        if has_key_and_store(&bt.value)
    ) || matches!(&ty.value, SingleType_::Base(bt)
        if has_key_and_store(&bt.value)
    );

    is_cap_name || is_cap_type
}

fn has_key_and_store(bt: &BaseType_) -> bool {
    matches!(bt, BaseType_::Apply(abilities, _, _)
        if abilities.has_ability_(Ability_::Key) && abilities.has_ability_(Ability_::Store)
    )
}

// ============================================================================
// 2. Unchecked Division (CFG-aware)
// ============================================================================

pub struct UncheckedDivisionVerifier;

pub struct UncheckedDivisionVerifierAI<'a> {
    info: &'a TypingProgramInfo,
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
        _cfg: &ImmForwardCFG,
        _init_state: &mut DivState,
    ) -> Option<Self::AI<'a>> {
        if context.attributes.is_test_or_test_only() {
            return None;
        }

        Some(UncheckedDivisionVerifierAI {
            info: context.info,
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

    fn exp_custom(
        &self,
        context: &mut Self::ExecutionContext,
        state: &mut Self::State,
        e: &Exp,
    ) -> Option<Vec<DivisorValue>> {
        use UnannotatedExp_ as E;

        match &e.exp.value {
            // Check for division operations
            E::BinopExp(lhs, op, rhs) => {
                let op_str = format!("{:?}", op.value);
                if op_str.contains("Div") || op_str.contains("Mod") {
                    // Check if divisor is validated
                    let divisor_safe = self.is_divisor_safe(state, rhs);
                    if !divisor_safe {
                        let msg = "Division or modulo operation without zero-check validation";
                        let help = "Add validation: `assert!(divisor != 0, E_DIVISION_BY_ZERO)`";
                        let d = diag!(
                            UNCHECKED_DIV_V2_DIAG,
                            (e.exp.loc, msg),
                            (rhs.exp.loc, help),
                        );
                        context.add_diag(d);
                    }
                }
            }
            // Track assertions that validate non-zero
            E::ModuleCall(call) => {
                if self.is_assert_call(call) {
                    // Track validated variables
                    self.track_validated_vars(state, &call.arguments);
                }
            }
            _ => {}
        }

        None
    }
}

impl UncheckedDivisionVerifierAI<'_> {
    fn is_divisor_safe(&self, state: &DivState, divisor: &Exp) -> bool {
        match &divisor.exp.value {
            // Constants are safe
            UnannotatedExp_::Value(_) => true,
            UnannotatedExp_::Constant(_) => true,
            // Validated variables are safe
            UnannotatedExp_::Move { var, .. } | UnannotatedExp_::Copy { var, .. } => {
                matches!(
                    state.locals.get(var),
                    Some(LocalState::Available(_, DivisorValue::Validated | DivisorValue::Constant))
                )
            }
            // Conservative: assume safe if complex expression
            _ => false,
        }
    }

    fn is_assert_call(&self, call: &ModuleCall) -> bool {
        let func_name = call.name.value().as_str();
        func_name == "assert" || func_name.contains("assert")
    }

    fn track_validated_vars(&self, state: &mut DivState, args: &[Exp]) {
        // Look for assert!(var != 0) or assert!(var > 0) patterns
        if let Some(condition) = args.first() {
            if let Some(var) = self.extract_validated_var(condition) {
                if let Some(local_state) = state.locals.get_mut(&var) {
                    if let LocalState::Available(loc, _) = local_state {
                        *local_state = LocalState::Available(*loc, DivisorValue::Validated);
                    }
                }
            }
        }
    }

    fn extract_validated_var(&self, condition: &Exp) -> Option<Var> {
        // Look for patterns like: var != 0, var > 0, 0 < var
        match &condition.exp.value {
            UnannotatedExp_::BinopExp(lhs, _op, rhs) => {
                // Check if comparing with zero
                if self.is_zero_value(rhs) {
                    self.extract_var(lhs)
                } else if self.is_zero_value(lhs) {
                    self.extract_var(rhs)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn is_zero_value(&self, exp: &Exp) -> bool {
        matches!(&exp.exp.value, UnannotatedExp_::Value(v)
            if matches!(&v.value, Value_::U8(0) | Value_::U64(0) | Value_::U128(0) | Value_::U256(_))
        )
    }

    fn extract_var(&self, exp: &Exp) -> Option<Var> {
        match &exp.exp.value {
            UnannotatedExp_::Move { var, .. } | UnannotatedExp_::Copy { var, .. } => Some(*var),
            UnannotatedExp_::BorrowLocal(_, var) => Some(*var),
            _ => None,
        }
    }
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
// 3. Oracle Price Taint Tracking
// ============================================================================

pub struct OraclePriceTaintVerifier;

pub struct OraclePriceTaintVerifierAI<'a> {
    info: &'a TypingProgramInfo,
}

/// Taint tracking for oracle prices
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum TaintValue {
    #[default]
    Unknown,
    Clean,
    Tainted(Loc), // Location where taint originated
}

pub struct TaintExecutionContext {
    diags: CompilerDiagnostics,
}

#[derive(Clone, Debug)]
pub struct TaintState {
    locals: BTreeMap<Var, LocalState<TaintValue>>,
}

impl SimpleAbsIntConstructor for OraclePriceTaintVerifier {
    type AI<'a> = OraclePriceTaintVerifierAI<'a>;

    fn new<'a>(
        context: &'a CFGContext<'a>,
        _cfg: &ImmForwardCFG,
        _init_state: &mut TaintState,
    ) -> Option<Self::AI<'a>> {
        if context.attributes.is_test_or_test_only() {
            return None;
        }

        Some(OraclePriceTaintVerifierAI {
            info: context.info,
        })
    }
}

impl SimpleAbsInt for OraclePriceTaintVerifierAI<'_> {
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

    fn call_custom(
        &self,
        context: &mut Self::ExecutionContext,
        state: &mut Self::State,
        loc: &Loc,
        return_ty: &Type,
        f: &ModuleCall,
        args: Vec<TaintValue>,
    ) -> Option<Vec<TaintValue>> {
        let func_name = f.name.value().as_str();

        // Check if this is an oracle price fetch
        if self.is_oracle_price_call(func_name) {
            // Return tainted value
            return Some(vec![TaintValue::Tainted(*loc)]);
        }

        // Check if tainted value is used in arithmetic
        if self.is_arithmetic_call(func_name) || self.is_math_operation(func_name) {
            for arg in &args {
                if let TaintValue::Tainted(taint_loc) = arg {
                    let msg = "Untrusted oracle price used in calculation without validation";
                    let help = "Validate price: `assert!(price > 0, E_INVALID_PRICE)`";
                    let d = diag!(
                        ORACLE_TAINT_DIAG,
                        (*loc, msg),
                        (*taint_loc, "Price originates here"),
                        (*loc, help),
                    );
                    context.add_diag(d);
                }
            }
        }

        None
    }
}

impl OraclePriceTaintVerifierAI<'_> {
    fn is_oracle_price_call(&self, func_name: &str) -> bool {
        func_name.contains("get_price") || func_name.contains("oracle")
    }

    fn is_arithmetic_call(&self, func_name: &str) -> bool {
        matches!(func_name, "mul" | "div" | "add" | "sub")
    }

    fn is_math_operation(&self, func_name: &str) -> bool {
        func_name.contains("calculate")
            || func_name.contains("compute")
            || func_name.contains("value")
    }
}

impl SimpleDomain for TaintState {
    type Value = TaintValue;

    fn new(_context: &CFGContext, locals: BTreeMap<Var, LocalState<Self::Value>>) -> Self {
        TaintState { locals }
    }

    fn locals_mut(&mut self) -> &mut BTreeMap<Var, LocalState<Self::Value>> {
        &mut self.locals
    }

    fn locals(&self) -> &BTreeMap<Var, LocalState<Self::Value>> {
        &self.locals
    }

    fn join_value(v1: &Self::Value, v2: &Self::Value) -> Self::Value {
        use TaintValue::*;
        match (v1, v2) {
            (Tainted(loc), _) | (_, Tainted(loc)) => Tainted(*loc),
            (Clean, Clean) => Clean,
            _ => Unknown,
        }
    }

    fn join_impl(&mut self, _other: &Self, _result: &mut JoinResult) {}
}

impl SimpleExecutionContext for TaintExecutionContext {
    fn add_diag(&mut self, diag: CompilerDiagnostic) {
        self.diags.add(diag)
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Return all Phase II lint descriptors
pub fn descriptors() -> &'static [&'static LintDescriptor] {
    &[
        &UNUSED_CAPABILITY_PARAM_V2,
        &UNCHECKED_DIVISION_V2,
        &ORACLE_PRICE_TAINT,
        &RESOURCE_LEAK,
    ]
}

/// Look up a Phase II lint descriptor by name
pub fn find_descriptor(name: &str) -> Option<&'static LintDescriptor> {
    descriptors().iter().copied().find(|d| d.name == name)
}

/// Create Abstract Interpreter visitors for all Phase II lints
///
/// These can be added to the Move compiler via:
/// ```ignore
/// compiler.add_visitors(absint_lints::create_visitors())
/// ```
pub fn create_visitors() -> Vec<Box<dyn AbstractInterpreterVisitor>> {
    vec![
        Box::new(UnusedCapabilityVerifier),
        Box::new(UncheckedDivisionVerifier),
        Box::new(OraclePriceTaintVerifier),
        // Note: ResourceLeakVerifier not yet implemented
    ]
}

// Implement AbstractInterpreterVisitor for each verifier
impl AbstractInterpreterVisitor for UnusedCapabilityVerifier {
    fn verify(&self, context: &CFGContext, cfg: &ImmForwardCFG) -> CompilerDiagnostics {
        UnusedCapabilityVerifier::verify(context, cfg)
    }
}

impl AbstractInterpreterVisitor for UncheckedDivisionVerifier {
    fn verify(&self, context: &CFGContext, cfg: &ImmForwardCFG) -> CompilerDiagnostics {
        UncheckedDivisionVerifier::verify(context, cfg)
    }
}

impl AbstractInterpreterVisitor for OraclePriceTaintVerifier {
    fn verify(&self, context: &CFGContext, cfg: &ImmForwardCFG) -> CompilerDiagnostics {
        OraclePriceTaintVerifier::verify(context, cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cap_value_join() {
        use CapValue::*;

        assert_eq!(CapState::join_value(&Used, &Used), Used);
        assert_eq!(CapState::join_value(&Used, &Unused), Used);
        assert_eq!(CapState::join_value(&Unused, &Unused), Unused);
    }

    #[test]
    fn test_divisor_value_join() {
        use DivisorValue::*;

        assert_eq!(DivState::join_value(&Validated, &Validated), Validated);
        assert_eq!(DivState::join_value(&Constant, &Constant), Constant);
        assert_eq!(DivState::join_value(&Validated, &Unknown), Unknown);
    }

    #[test]
    fn test_taint_value_join() {
        use TaintValue::*;

        assert_eq!(TaintState::join_value(&Clean, &Clean), Clean);
        
        let tainted = Tainted(Loc::invalid());
        assert_eq!(
            TaintState::join_value(&tainted, &Clean),
            tainted
        );
    }
}
