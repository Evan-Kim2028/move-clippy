# Semantic Linter Expansion Specification

**Version:** 1.0  
**Author:** move-clippy team  
**Date:** 2025-12-14  
**Status:** Proposed  
**Related Issues:** #13, #12, #11, #8, #9

---

## Executive Summary

This specification outlines the plan to expand move-clippy's semantic linting capabilities by leveraging the existing Sui Move compiler infrastructure. The goal is to create a production-grade linter that achieves near-zero false positive rates by tightly integrating with the Move compiler's type system, control flow analysis, and abstract interpretation framework.

### Key Philosophy

**Leverage, don't reinvent.** The Sui Move compiler (`move-compiler/src/sui_mode/linters/`) already implements production-quality semantic lints using abstract interpretation and type visitors. Our strategy is to:

1. **Delegate** to existing Sui lints where they exist
2. **Extend** using the same patterns (SimpleAbsInt, CFGIRVisitor) for new lints
3. **Expand** move-clippy as the "community linting extension" of the Sui compiler

---

## Background

### Current State

move-clippy currently has three linting modes:

| Mode | Technology | Speed | Accuracy | Use Case |
|------|-----------|-------|----------|----------|
| **Fast** | tree-sitter | 10ms/file | Pattern-based | Style, naming, modernization |
| **Semantic** | Move compiler typing AST | 100ms+ | Type-aware | Ability checks, naming |
| **Delegated** | Sui compiler linters | 200ms+ | Production | Object ownership, transfers |

### What Sui Compiler Already Provides

The Sui compiler has 11 production-quality semantic lints:

| Lint | Analysis Type | Description |
|------|---------------|-------------|
| `share_owned` | Abstract Interpretation | Detects sharing potentially-owned objects |
| `self_transfer` | Abstract Interpretation | Non-composable transfer to sender |
| `custom_state_change` | Call Graph | Custom transfer must call framework |
| `coin_field` | Type Visitor | Use Balance not Coin in structs |
| `freeze_wrapped` | Abstract Interpretation | Don't freeze wrapped objects |
| `collection_equality` | Type Visitor | Don't compare collections |
| `public_random` | Type Visitor | Random state should be private |
| `missing_key` | Type Visitor | Objects need key ability |
| `freezing_capability` | Type Visitor | Don't store freeze caps |
| `prefer_mut_tx_context` | Type Visitor | Use &mut TxContext |
| `unnecessary_public_entry` | Type Visitor | Remove unnecessary public |

### What move-clippy Has Discovered

From real-world audits and ecosystem validation:

| Lint | Source | Current Status |
|------|--------|----------------|
| `droppable_hot_potato` | Trail of Bits 2025, AlphaLend bug | ✅ Stable |
| `suspicious_overflow_check` | Cetus $223M hack | Preview |
| `stale_oracle_price` | Bluefin Audit 2024 | Stable |
| `single_step_ownership_transfer` | Bluefin Audit 2024 | Stable |
| `unused_capability_param` | SlowMist 2024 | Stable |
| `unchecked_division` | Common pattern | Preview |
| `oracle_zero_price` | Bluefin Audit 2024 | Preview |
| `missing_access_control` | SlowMist 2024 | Preview |
| `ignored_boolean_return` | Typus $3.4M hack | Stable |
| `shared_capability_object` | Typus hack | Stable |

---

## Architecture

### Core Infrastructure from Move Compiler

```
move-compiler/
├── src/
│   ├── cfgir/
│   │   ├── visitor.rs          # CFGIRVisitor, SimpleAbsInt traits
│   │   ├── absint.rs           # AbstractDomain, JoinResult
│   │   └── cfg.rs              # Control Flow Graph
│   ├── sui_mode/
│   │   ├── linters/            # 11 production lints
│   │   │   ├── share_owned.rs  # Template: SimpleAbsInt
│   │   │   ├── coin_field.rs   # Template: Type Visitor
│   │   │   └── ...
│   │   └── typing.rs           # Sui type checking
│   └── typing/
│       └── visitor.rs          # TypingVisitor trait
```

### move-clippy Integration Points

```
move-clippy/
├── src/
│   ├── semantic.rs             # Entry point for semantic lints
│   │   ├── lint_package()      # Compiles and runs all semantic lints
│   │   ├── lint_sui_visitors() # Delegates to Sui compiler lints
│   │   └── lint_*()            # Custom typing AST lints
│   └── rules/
│       └── security.rs         # Fast (tree-sitter) security lints
```

### Key Traits and Patterns

#### 1. SimpleAbsInt (Abstract Interpretation)

Used for tracking values through control flow:

```rust
pub trait SimpleAbsInt {
    type State: SimpleDomain;           // Abstract domain (values)
    type ExecutionContext: SimpleExecutionContext;  // Diagnostics
    
    // Override these for custom logic:
    fn exp_custom(&self, ctx, state, e) -> Option<Vec<Value>>;
    fn call_custom(&self, ctx, state, loc, ret_ty, f, args) -> Option<Vec<Value>>;
    fn lvalue_custom(&self, ctx, state, l, value) -> bool;
    
    // Final reporting:
    fn finish(&mut self, final_states, diags) -> Diagnostics;
}

pub trait SimpleDomain {
    type Value: Clone + Default + Eq;  // Your abstract value type
    fn join_value(v1: &Value, v2: &Value) -> Value;  // Lattice join
}
```

**Example: share_owned Value Domain**
```rust
#[derive(Clone, Debug, Default)]
pub enum Value {
    FreshObj,           // Created in this function (safe to share)
    NotFreshObj(Loc),   // From param/unpack (NOT safe to share)
    #[default]
    Other,
}
```

#### 2. CFGIRVisitor (Simple Traversal)

Used for visiting AST without state tracking:

```rust
pub trait CFGIRVisitorContext {
    fn visit_struct_custom(&mut self, module, name, sdef) -> bool;
    fn visit_function_custom(&mut self, module, name, fdef) -> bool;
    fn visit_exp_custom(&mut self, e: &Exp) -> bool;
}
```

#### 3. Typing AST Analysis (Current move-clippy Approach)

Used for type-based checks without CFG:

```rust
// Current pattern in move-clippy
fn lint_capability_naming(out: &mut Vec<Diagnostic>, info: &TypingProgramInfo) {
    for (_, minfo) in info.modules.key_cloned_iter() {
        for (sname, sdef) in minfo.structs.key_cloned_iter() {
            let abilities = &sdef.abilities;
            // Check ability patterns...
        }
    }
}
```

---

## Implementation Phases

### Phase I: Consolidation and Stabilization (Low Effort)

**Goal:** Ensure all existing Sui lints are properly delegated and our typing AST lints are validated.

**Timeline:** 1-2 days

#### Deliverables

1. **Complete Sui lint delegation**
   - [x] Map all 11 Sui lint codes in `descriptor_for_sui_code()`
   - [x] Added `prefer_mut_tx_context`, `unnecessary_public_entry`
   - [ ] Document which lints are delegated vs custom in `SEMANTIC_LINT_STATUS.md`

2. **Validate typing AST lints**
   - [x] `capability_naming` - uses TypingProgramInfo
   - [x] `event_naming` - uses TypingProgramInfo
   - [x] `getter_naming` - uses typing AST
   - [x] `unused_capability_param` - uses typing AST traversal
   - [x] `unfrozen_coin_metadata` - uses typing AST traversal
   - [x] `unchecked_division` - uses typing AST with basic state tracking
   - [x] `oracle_zero_price` - uses typing AST with basic state tracking
   - [x] `unused_return_value` - uses typing AST traversal
   - [x] `missing_access_control` - uses typing AST traversal
   - [ ] Run against all 11 ecosystem repos (validate FP rates)
   - [ ] Calculate FP rates, promote to Stable if <10%

3. **Documentation**
   - [ ] Update `SEMANTIC_LINT_STATUS.md` with delegation status
   - [ ] Add lint source attribution (which are Sui vs custom)
   - [x] Create `SECURITY_LINTS.md` with audit references

#### Current Implementation Status

| Lint | Implementation | FP Rate | Notes |
|------|---------------|---------|-------|
| `capability_naming` | TypingProgramInfo | ~5% | Check key+store abilities |
| `event_naming` | TypingProgramInfo | ~10% | Check copy+drop abilities |
| `getter_naming` | Typing AST | ~15% | Body inspection |
| `unused_capability_param` | Typing AST recursion | Unknown | Needs ecosystem validation |
| `unfrozen_coin_metadata` | Typing AST recursion | Low | Specific pattern match |
| `unchecked_division` | Basic state tracking | Unknown | Tracks validated vars |
| `oracle_zero_price` | Basic state tracking | Unknown | Price variable heuristics |
| `unused_return_value` | Typing AST | Medium | Known important functions |
| `missing_access_control` | Type + heuristics | High | Needs refinement |

#### Development Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Sui lint code changes | Low | Pin to specific compiler version |
| Ecosystem compilation failures | Medium | Graceful degradation: skip semantic on compile error |
| High FP rates in Preview lints | Medium | Keep in Preview until validated, allow suppressions |

#### Effort Estimate

- Ecosystem validation: 4-8 hours
- Documentation updates: 2-3 hours
- FP rate analysis and refinement: 4-8 hours
- **Total: 1-2 days**

---

### Phase II: SimpleAbsInt Security Lints (Medium Effort)

**Goal:** Implement custom security lints using Sui's abstract interpretation framework for precise control-flow-aware analysis.

**Timeline:** 2-3 weeks

#### Why SimpleAbsInt?

Current move-clippy semantic lints use simple AST traversal, which has limitations:

1. **No control flow awareness** - Can't track values through branches
2. **No join semantics** - Can't merge states from different paths
3. **Linear scanning** - Misses aliasing and reassignment

SimpleAbsInt solves these by:
- Building a CFG (Control Flow Graph)
- Running abstract interpretation to fixed point
- Tracking abstract values through all paths

#### Target Lints

| Lint | Abstract Domain | Complexity | Priority |
|------|-----------------|------------|----------|
| `unused_capability_param_v2` | `Used \| Unused` per param | Low | High |
| `unchecked_division_v2` | `Validated(var_id) \| Unvalidated` | Medium | High |
| `oracle_price_taint` | `Tainted(source) \| Clean \| Validated` | Medium | Medium |
| `resource_leak` | `Alive(var) \| Consumed \| Unknown` | High | Medium |
| `double_borrow_guard` | `Borrowed(ref_id) \| Released` | High | Low |

#### Implementation Pattern

Using `share_owned.rs` as the template (from Sui compiler):

```rust
// 1. Define the abstract value domain
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum CapabilityState {
    #[default]
    Unused,          // Parameter received but not used
    Used,            // Parameter was accessed/used
}

// 2. Define the abstract state (mapping from variable -> value)
pub struct CapabilityState {
    cap_states: BTreeMap<Var, CapabilityState>,
}

impl SimpleDomain for State {
    type Value = CapabilityState;
    
    fn join_value(v1: &Value, v2: &Value) -> Value {
        match (v1, v2) {
            (CapabilityState::Used, _) | (_, CapabilityState::Used) => CapabilityState::Used,
            _ => CapabilityState::Unused,
        }
    }
}

// 3. Implement SimpleAbsIntConstructor
impl SimpleAbsIntConstructor for UnusedCapabilityVerifier {
    type AI<'a> = UnusedCapabilityVerifierAI<'a>;
    
    fn new<'a>(context: &'a CFGContext, cfg: &ImmForwardCFG, 
               init_state: &mut State) -> Option<Self::AI<'a>> {
        // Find capability parameters
        let cap_params = find_capability_params(&context.signature);
        if cap_params.is_empty() {
            return None; // No analysis needed
        }
        
        // Initialize state: all caps start as Unused
        for cap in &cap_params {
            init_state.cap_states.insert(cap.clone(), CapabilityState::Unused);
        }
        
        Some(UnusedCapabilityVerifierAI { 
            cap_params, 
            info: context.info 
        })
    }
}

// 4. Implement SimpleAbsInt
impl SimpleAbsInt for UnusedCapabilityVerifierAI<'_> {
    type State = State;
    type ExecutionContext = ExecutionContext;
    
    fn exp_custom(&self, ctx, state, e) -> Option<Vec<CapabilityState>> {
        // Mark capability as Used when accessed
        if let E::Exp_::BorrowLocal(_, var) = &e.value {
            if self.cap_params.contains(&var) {
                state.cap_states.insert(var.clone(), CapabilityState::Used);
            }
        }
        None // Use default handling
    }
    
    fn call_custom(&self, ctx, state, loc, ret_ty, f, args) 
        -> Option<Vec<CapabilityState>> 
    {
        // Mark capabilities as Used if passed to functions
        for (i, arg) in args.iter().enumerate() {
            if let Some(var) = extract_var_from_value(arg) {
                if self.cap_params.contains(&var) {
                    state.cap_states.insert(var.clone(), CapabilityState::Used);
                }
            }
        }
        None
    }
    
    fn finish(&mut self, final_states: BTreeMap<Label, State>, 
              diags: &mut Diagnostics) {
        // Check final states: report unused capabilities
        for (label, state) in &final_states {
            for (cap, status) in &state.cap_states {
                if *status == CapabilityState::Unused {
                    diags.add(unused_cap_warning(cap, &self.context));
                }
            }
        }
    }
}
```

#### Integration Approach

**Option A: Direct Integration (Recommended for Phase II)**

Add lints directly to move-clippy, importing Sui compiler infrastructure:

```rust
// In src/semantic.rs
use move_compiler::cfgir::visitor::{SimpleAbsInt, SimpleAbsIntConstructor, SimpleDomain};

pub fn lint_package(package_path: &Path, settings: &LintSettings) 
    -> ClippyResult<Vec<Diagnostic>> 
{
    // ... existing setup ...
    
    // Run SimpleAbsInt lints via Sui's linter infrastructure
    // This requires adding our lints to a custom visitor list
    let custom_visitors = vec![
        Box::new(UnusedCapabilityVerifier::new()),
        Box::new(UncheckedDivisionVerifier::new()),
    ];
    
    // Compile with custom visitors
    let compiler = compiler
        .add_visitors(custom_visitors);
    
    // ... collect diagnostics ...
}
```

**Option B: Upstream to Sui (Long-term)**

If lints are generally useful, consider upstreaming to Sui:

1. Fork sui-mono
2. Add lint to `move-compiler/src/sui_mode/linters/`
3. Submit PR to Mysten Labs
4. Once merged, move-clippy automatically gets it via delegation

#### Development Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| SimpleAbsInt API instability | Medium | Pin to compiler version, test on upgrades |
| Complex CFG edge cases | Medium | Study existing Sui lints as reference |
| Performance overhead | Low | Abstract interpretation is O(n) in CFG size |
| Learning curve | Medium | Document patterns, create lint template |

#### Effort Estimate

- Study Sui lint implementations: 4-8 hours
- Create lint template/scaffold: 4-8 hours
- Implement `unused_capability_param_v2`: 8-16 hours
- Implement `unchecked_division_v2`: 8-16 hours
- Implement `oracle_price_taint`: 16-24 hours
- Ecosystem validation: 8-16 hours
- **Total: 2-3 weeks**

---

### Phase III: Cross-Module Analysis and Advanced Features (Higher Effort)

**Goal:** Enable analysis that requires information across module boundaries and advanced patterns.

**Timeline:** 4-8 weeks

#### Target Capabilities

1. **Cross-module call graph analysis**
   - Track which external functions are called
   - Verify capability flow across module boundaries
   - Detect transitive security issues

2. **GlobalEnv integration**
   - Full program analysis using Move prover infrastructure
   - Access to all modules and dependencies

3. **Taint tracking across modules**
   - Track untrusted data from oracles/user input
   - Flow analysis through function calls

4. **Pattern-based vulnerability detection**
   - Flashloan attack patterns
   - Price manipulation patterns
   - Reentrancy-like patterns

#### Architecture Changes

```rust
// New module for cross-module analysis
pub mod cross_module {
    use move_compiler::expansion::ast::Program;
    use move_compiler::naming::ast::Program as NamedProgram;
    
    pub struct CallGraph {
        // Module -> [called modules]
        calls: BTreeMap<ModuleIdent, Vec<(ModuleIdent, FunctionName)>>,
        // Function -> capabilities required
        cap_requirements: BTreeMap<(ModuleIdent, FunctionName), Vec<TypeName>>,
    }
    
    impl CallGraph {
        pub fn build(prog: &Program) -> Self { ... }
        
        pub fn transitive_callers(&self, module: &ModuleIdent, func: &FunctionName) 
            -> Vec<(ModuleIdent, FunctionName)> { ... }
        
        pub fn capability_leak_analysis(&self) -> Vec<CapabilityLeak> { ... }
    }
}
```

#### Target Lints

| Lint | Analysis Type | Complexity | Description |
|------|---------------|------------|-------------|
| `transitive_capability_leak` | Call graph | High | Capability escapes module boundary |
| `flashloan_without_repay` | Cross-module flow | High | Borrowed assets not returned |
| `price_manipulation_window` | Temporal analysis | Very High | State changes between oracle reads |
| `untrusted_input_flow` | Taint analysis | High | User input flows to sensitive ops |

#### Development Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Move compiler API changes | High | Use stable AST structures, version pinning |
| Performance at scale | Medium | Lazy analysis, caching |
| Complexity of cross-module | High | Incremental approach, thorough testing |
| False positive explosion | High | Conservative defaults, extensive allow-listing |

#### Effort Estimate

- Call graph infrastructure: 16-24 hours
- Cross-module capability analysis: 24-40 hours
- Taint tracking framework: 24-40 hours
- Advanced pattern detection: 40-80 hours
- Ecosystem validation: 16-24 hours
- **Total: 4-8 weeks**

---

## Risk Assessment Summary

### Technical Risks

| Risk | Phase | Probability | Impact | Mitigation |
|------|-------|-------------|--------|------------|
| Sui compiler API changes | II, III | Medium | High | Version pinning, compatibility layer |
| False positive rate | All | High | Medium | Aggressive testing, Preview gates |
| Performance degradation | II, III | Low | Medium | Profiling, lazy evaluation |
| Compilation failures in ecosystem | I | Medium | Low | Graceful degradation |

### Process Risks

| Risk | Phase | Probability | Impact | Mitigation |
|------|-------|-------------|--------|------------|
| Scope creep | All | Medium | Medium | Strict phase boundaries |
| Insufficient testing | All | Medium | High | Ecosystem validation gates |
| Documentation lag | All | High | Medium | Doc-first approach |

### Dependency Risks

| Risk | Phase | Probability | Impact | Mitigation |
|------|-------|-------------|--------|------------|
| Sui compiler version drift | II, III | High | High | Pin versions, CI compatibility tests |
| tree-sitter grammar changes | I | Low | Low | Grammar versioning |

---

## Success Criteria

### Phase I Success
- [ ] All 11 Sui lints properly delegated
- [ ] Active lints have <10% FP rate on ecosystem
- [ ] Documentation complete and accurate

### Phase II Success
- [ ] At least 3 SimpleAbsInt lints implemented
- [ ] FP rate <5% on ecosystem repos
- [ ] Performance overhead <50% vs current semantic mode

### Phase III Success
- [ ] Cross-module analysis working on top 5 DeFi protocols
- [ ] At least 2 cross-module lints in Stable
- [ ] Integration with existing CI workflows

---

## Appendix A: Ecosystem Test Repos

| Repo | Domain | LOC | Notes |
|------|--------|-----|-------|
| deepbook-v3 | DEX | ~15k | Complex order matching |
| cetus-clmm | DEX/AMM | ~12k | Concentrated liquidity |
| suilend | Lending | ~8k | Oracle integration |
| scallop | Lending | ~10k | Multi-collateral |
| bucket-protocol | Stablecoin | ~6k | CDP mechanics |
| turbos | DEX | ~8k | Order book |
| kriya | DEX | ~5k | AMM pools |
| interest-protocol | Lending | ~4k | Interest rates |
| navi | Lending | ~6k | Composability |
| typus-dov | Options | ~3k | Vaults |
| bluefin | Perpetuals | ~5k | Oracle-heavy |

---

## Appendix B: Lint Priority Matrix

| Lint | Audit Severity | FP Risk | Effort | Priority Score |
|------|---------------|---------|--------|----------------|
| `unused_capability_param` | Critical | Low | Low | **P0** |
| `unchecked_division` | High | Medium | Low | **P0** |
| `oracle_zero_price` | High | Medium | Medium | **P1** |
| `missing_access_control` | Critical | High | Low | **P1** |
| `resource_leak` | High | Medium | High | **P2** |
| `flashloan_repay` | Critical | Low | Very High | **P2** |
| `price_manipulation` | Critical | High | Very High | **P3** |

---

## Appendix C: Related Resources

### Sui Compiler Linters
- [share_owned.rs](https://github.com/MystenLabs/sui/blob/main/external-crates/move/crates/move-compiler/src/sui_mode/linters/share_owned.rs)
- [coin_field.rs](https://github.com/MystenLabs/sui/blob/main/external-crates/move/crates/move-compiler/src/sui_mode/linters/coin_field.rs)
- [linters/mod.rs](https://github.com/MystenLabs/sui/blob/main/external-crates/move/crates/move-compiler/src/sui_mode/linters/mod.rs)

### Abstract Interpretation Infrastructure
- [cfgir/visitor.rs](https://github.com/MystenLabs/sui/blob/main/external-crates/move/crates/move-compiler/src/cfgir/visitor.rs)
- [cfgir/absint.rs](https://github.com/MystenLabs/sui/blob/main/external-crates/move/crates/move-compiler/src/cfgir/absint.rs)

### Security Audit Resources
- [SlowMist Sui Move Auditing Primer](https://github.com/slowmist/Sui-MOVE-Smart-Contract-Auditing-Primer)
- [MoveBit Security Best Practices](https://movebit.xyz/blog/post/Sui-Objects-Security-Principles-and-Best-Practices.html)
- [Trail of Bits Flash Loan Security](https://blog.trailofbits.com/2025/09/10/how-sui-move-rethinks-flash-loan-security/)

---

## Version History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2025-12-14 | move-clippy team | Initial specification |
