# GPT 5.2 Comprehensive Overview (move-clippy)

**Workspace:** `learning_move/packages/move-clippy/`  
**Document purpose:** Preserve a detailed, durable analysis of the move-clippy library’s architecture, its lint development framework, and how it relates to the Sui Move linter / Move compiler, with concrete, actionable improvement recommendations focused on (1) refactoring for clarity and correctness, (2) test quality, and (3) building “zero false positive” lints before running on ecosystem packages.  
**Scope:** This document reflects the code as it exists in this checkout. It focuses on the engine/framework and on “lint correctness” methodology (stable-tier reliability), rather than proposing new lints.

---

## Status Update (2025-12-18)

This document was initially written against an earlier iteration of the codebase. Since then, a larger refactor landed that materially changes the “lint framework” architecture and resolves several correctness issues.

### Completed work (high impact)

1. **Unified registry / single source of truth**
   - Introduced a cached unified registry (`src/unified.rs`) as the common inventory for `list-rules`, `explain`, and other tooling.
   - Removed the older name-list / name→tier sources of truth (`FAST_LINT_NAMES`, `SEMANTIC_LINT_NAMES`, `get_lint_group(...)`) and rewired filtering to operate on descriptors.

2. **Bulletproof suppression (fast mode)**
   - Eliminated suppression bypasses by removing remaining `ctx.report_diagnostic(...)` call sites under `src/rules/` (they bypass item/module directives).
   - Added a regression meta-test that fails if `ctx.report_diagnostic(` reappears under `src/rules/` (`tests/meta_invariants.rs`).

3. **Full-mode directive system (compiler-valid)**
   - Implemented full-mode directive evaluation as a single post-pass over diagnostics in `semantic::lint_package`.
   - Added a compiler-valid directive encoding via `ext` attributes:
     - `#[ext(move_clippy(allow(<name|category>)))]`
     - `#[ext(move_clippy(deny(<name|category>)))]`
     - `#[ext(move_clippy(expect(<name|category>)))]`
   - `expect` is enforced as a testing invariant and produces `unfulfilled_expectation` if unmet.

4. **Tree-sitter hardening around directive syntax**
   - Added a pre-parse “masking” step that replaces directive lines with whitespace of identical byte-length (preserves spans) so the tree-sitter AST stays clean.
   - Expanded tests to cover `#![allow|deny|expect(...)]`, `ext(move_clippy(...))`, and whitespace-heavy formatting (`src/parser.rs`).

5. **Snapshot test clarity**
   - Renamed the old “semantic snapshots” to reflect they were syntactic (`tests/syntactic_snapshots.rs`).
   - Added feature-gated true semantic package snapshots that call `semantic::lint_package` (`tests/semantic_package_snapshots.rs`).

### Additional correctness/quality fixes

- **Full-mode build isolation:** `semantic::lint_package` isolates Move build artifacts in a temporary install directory to avoid fixture mutation and parallel-test races.
- **Directive anchoring bug fix:** suppression anchors no longer incorrectly treat nodes like `function_identifier` as item anchors (`src/suppression.rs`).
- **Duplicate diagnostic cleanup:** semantic results are de-duplicated before returning (prevents double-reporting in some compiler/linter paths).
- **Documentation tightening:** `docs/STABILITY.md` is policy-only (no hard-coded inventories), suppression examples use correct syntax, and historical inventory snapshots live under `docs/notes/` (with current catalogs generated via `docs/LINT_REFERENCE.md` and `docs/LINT_CATALOG_SUMMARY.md`).
- **Semantic spec test harness dedup:** shared helper utilities for semantic spec tests live under `tests/support/` (reduces repeated temp-package boilerplate across `tests/*_spec.rs`).
- **Fixture documentation strengthening:** `tests/fixtures/README.md` now documents phase directories, WIP fixture expectations, and when/why to use test-only Sui shims.
- **Developer guide drift fix:** `docs/LINT_DEVELOPMENT_GUIDE.md` no longer references a non-existent `tests/false_positive_prevention.rs` and instead documents the current snapshot/spec harnesses.

### Next low-hanging fruit (good follow-ups)

- Add a CLI `--sui-only` mode/flag to run just delegated Sui lints (the test suite already demonstrates the usefulness of this separation).
- Add an invariant test that enforces “analysis kind ↔ required flag” consistency (e.g., `CrossModule` lints must not claim `--preview` in docs/metadata when they’re gated by `--experimental`).
- Expand “zero FP” spec tests for semantic lints using mutation-style harnesses (mutants that violate an obligation should always be caught).
- Add a small “directive conformance” doc page that shows fast vs full syntax side-by-side, with compiler-valid examples for `module` scope and item scope.

---

## Table of Contents

1. Executive Summary
2. Repository and Feature Model
3. High-Level Architecture (Phases and Modes)
4. Core Data Model (Descriptors, Tiers, Analysis Kinds)
5. Fast Mode Pipeline (Tree-sitter)
6. Full Mode Pipeline (Move compiler, typing, Sui lints)
7. CFG / Abstract Interpretation (Phase II)
8. Cross-Module Analysis (Phase III)
9. Unified Registry (Cross-phase metadata view)
10. Lint Development Workflow (How to add lints)
11. Suppressions and Annotations (Current vs intended)
12. Testing Architecture (What exists, what’s missing)
13. “Zero False Positives” Methodology
14. Issues and Improvement Opportunities (Refactoring + Testing)
15. Recommended Roadmap (Practical sequencing)

---

## 1. Executive Summary

move-clippy is structured as a **two-mode linter** with an internal notion of “phases”:

- **Fast mode:** A tree-sitter-based parser + a set of syntactic lints that run per-file with no compilation (`src/lib.rs` and `src/lint.rs`).
- **Full mode:** A Move compiler-based pipeline that compiles a **Move package** (Sui flavor), extracts typing artifacts, runs:
  - move-clippy’s type-based lints,
  - CFG/abstract interpretation visitors (Phase II),
  - cross-module analyses (Phase III),
  - and **delegated Sui lints** from `move_compiler::sui_mode::linters`,
  producing move-clippy diagnostics (`src/semantic.rs`).

The project already contains strong ingredients for high lint quality (tiering system, a growing set of spec-driven tests, ecosystem baseline testing infrastructure). However, there are important **architectural inconsistencies** that directly affect “zero false positives” goals:

- Multiple sources of truth for lint metadata and gating (tier lists vs descriptor fields vs name-based maps).
- Preview/experimental/deprecated gating not uniform across CLI/config/full mode.
- A richer annotation system exists but is not integrated; suppression is currently a simpler mechanism.
- Some tests labeled “semantic” are not actually using the semantic pipeline (they run the syntactic `LintEngine`).

The highest leverage improvements are:

1. Move to a **single source of truth** for lint metadata and tier gating (derive from descriptors / a unified registry).
2. Make preview/experimental/deprecated gating consistent across CLI, config, and full-mode execution.
3. Tighten “stable tier” quality by enforcing structural matching (not substring heuristics) and expanding **spec-driven + mutation-based** test coverage for semantic/CFG lints.

---

## 2. Repository and Feature Model

### 2.1 Where this lives

This is its own repository under:

- `learning_move/packages/move-clippy/`

### 2.2 Cargo features and dependencies

`Cargo.toml` defines:

- `default = ["telemetry"]`
- `full = ["move-compiler", "move-ir-types", "move-package"]`

The `full` feature currently depends on a Git snapshot for compiler crates (see `Cargo.toml`). This is a deliberate choice to access Move compiler internals (typing AST, typing program info, Sui linters).

**Implication:** Full mode is inherently tied to compiler API stability and crate versions. This should inform how you isolate compiler-integration code and how you test/CI it.

---

## 3. High-Level Architecture (Phases and Modes)

### 3.1 Two user-facing modes

The CLI exposes `--mode fast|full` (`src/cli.rs`):

- **Fast** (default): parse `.move` sources and run syntactic rules.
- **Full**: compile a Move package and run semantic rules + delegated Sui lints.

### 3.2 Internal “phases” (conceptual layers)

The codebase and docs describe a phased architecture (see `src/unified.rs`):

1. **Phase I (Syntactic):** tree-sitter + `LintRule` trait (`src/lint.rs`, `src/lib.rs`, `src/rules/*`).
2. **Phase II (Semantic / Typing):** type-based lints implemented in `src/semantic.rs` that use typing AST / program info.
3. **Phase II (CFG / AbsInt):** control-flow-aware analyses via compiler “SimpleAbsInt” visitors (`src/absint_lints.rs`), integrated through the compilation driver (`src/semantic.rs`).
4. **Phase III (Cross-Module):** call-graph / whole-program-ish analyses (`src/cross_module_lints.rs`), invoked from semantic pipeline.

The unified registry (`src/unified.rs`) attempts to provide a single metadata view over these phases.

---

## 4. Core Data Model (Descriptors, Tiers, Analysis Kinds)

### 4.1 LintDescriptor: the canonical lint metadata

`src/lint.rs` defines `LintDescriptor` (static metadata):

- `name: &'static str`
- `category: LintCategory`
- `description: &'static str`
- `group: RuleGroup` (tier/stability)
- `fix: FixDescriptor` (availability + safety)
- `analysis: AnalysisKind` (syntactic / type-based / CFG / cross-module)
- `gap: Option<TypeSystemGap>` (taxonomy for “what gap this lint addresses”)

This is the right place to store “truth” about a lint.

### 4.2 RuleGroup (tiering / stability)

`RuleGroup` is a 4-tier model (`src/lint.rs`):

- `Stable`: intended to have ~zero false positives and be enabled by default.
- `Preview`: opt-in; should have strong accuracy but still gathering validation.
- `Experimental`: opt-in; used for research/audits; higher FP risk.
- `Deprecated`: compatibility; should generally not emit diagnostics.

The CLI exposes this through `--preview`, `--experimental`, and `--show-tier` (`src/cli.rs`).

### 4.3 AnalysisKind (how the lint reasons)

`AnalysisKind` (`src/lint.rs`) indicates required tooling and expected accuracy:

- `Syntactic` (tree-sitter)
- `TypeBased` (typing artifacts from compiler)
- `TypeBasedCFG` (CFG-aware analysis, typically via abstract interpretation)
- `CrossModule` (call-graph / multi-module analysis)

This is important because “zero FP” is not an intrinsic property of a lint category (security/style), but of its analysis precision + guardrails.

### 4.4 FixDescriptor + FixSafety

Fix metadata is stored in `FixDescriptor` (`src/lint.rs`) with:

- `available: bool`
- `safety: FixSafety::{Safe, Unsafe}`
- `description`

This is aligned with the “Ruff-style” approach: safe fixes are machine-applicable by default; unsafe fixes require explicit opt-in.

---

## 5. Fast Mode Pipeline (Tree-sitter)

### 5.1 Engine flow

Fast mode runs through `LintEngine` (`src/lib.rs`):

1. Parse with tree-sitter (`parse_source`, `src/parser.rs`).
2. Create a `LintContext` (tracks settings + diagnostics, `src/lint.rs`).
3. For each registered rule (`LintRegistry`), call `rule.check(root, source, &mut ctx)`.

### 5.2 Fast lint authoring style

Fast rules are implemented under:

- `src/rules/style.rs`
- `src/rules/modernization.rs`
- `src/rules/security.rs`
- `src/rules/test_quality.rs`
- `src/rules/conventions.rs`

Each lint:

- defines a `static` `LintDescriptor`,
- defines a `struct SomeLint;`,
- implements `LintRule` and then typically calls a recursive helper that:
  - checks current node kind / extracts text,
  - reports a node span,
  - recurses into children.

### 5.3 Known risks in fast mode

Fast mode is inherently “no type info”. The most common causes of false positives in syntactic linters are:

- substring matching over raw node text (matches comments, strings, unrelated contexts),
- heuristic naming assumptions (“ends_with Cap”, “contains admin”),
- failing to validate local context (e.g., ensuring you’re in call position rather than a string literal),
- insufficient negative tests.

Where the lint is stable-tier, it should be conservative:

- prefer structural matches (tree-sitter fields, call callee path extraction),
- prefer exact matches and scope checks,
- accept false negatives over false positives.

---

## 6. Full Mode Pipeline (Move compiler, typing, Sui lints)

### 6.1 Entry point: semantic::lint_package

`semantic::lint_package` is feature-gated (`src/semantic.rs`) and only available with `--features full`.

At a high level (`src/semantic.rs`):

1. Canonicalize the package root.
2. Use `move_package::BuildConfig` and set Sui flavor:
   - `build_config.default_flavor = Some(Flavor::Sui)`
3. Construct a `BuildPlan` from the resolution graph.
4. Create a `SaveHook` to retain typing artifacts:
   - `SaveFlag::Typing`
   - `SaveFlag::TypingInfo`
5. Create and register Phase II visitors (AbsInt) if `preview` is enabled.
6. Compile with a driver that:
   - installs Sui linter known filters,
   - adds the save hook,
   - adds visitors,
   - captures warnings into memory (important for `--format json` correctness).
7. Retrieve:
   - `typing_ast: typing::ast::Program`
   - `typing_info: Arc<TypingProgramInfo>`
8. Run move-clippy semantic lints, AbsInt conversion, cross-module analyses, and Sui delegated lints.
9. Filter out preview/experimental diagnostics if preview is disabled.

### 6.2 Delegating Sui lints

The Sui lints are sourced from:

- `move_compiler::sui_mode::linters`

move-clippy defines wrapper descriptors (e.g., `SHARE_OWNED`, `SELF_TRANSFER`, etc.) in `src/semantic.rs` and maps Sui diagnostic codes to those descriptors.

**Key design choice:** Delegation allows move-clippy to provide unified output formatting and triage tooling while relying on upstream lints for certain Sui-specific correctness rules.

### 6.3 Why compilation-based lints are the right “zero FP” foundation

If your goal is “zero false positives” before ecosystem testing, the best approach is:

- stable-tier security lints should be **type/ability/IR grounded**,
- CFG-tier lints should be based on compiler CFG and dominance/value tracking,
- any lint that depends on protocol-specific business logic should remain non-stable (preview/experimental) unless it can be expressed as a type-system / API invariance rule.

The current codebase already follows this philosophy in parts (e.g., ability-based lints like hot potato variants).

---

## 7. CFG / Abstract Interpretation (Phase II)

### 7.1 Where it lives

CFG-aware analyses are implemented in `src/absint_lints.rs`.

This module uses Move compiler’s CFG IR and visitor framework:

- `cfgir::visitor::SimpleAbsInt`
- `AbstractInterpreterVisitor`
- domain/state types (`SimpleDomain`, `LocalState`, etc.)

### 7.2 Integration mechanism

Visitors are created via a helper (`absint_lints::create_visitors(preview)`) and passed into the compiler build in `semantic::lint_package`.

Diagnostics emitted by Phase II visitors are compiler diagnostics with a custom prefix/category and are converted back into move-clippy `Diagnostic` objects. `semantic::lint_package` includes a guard to avoid misclassifying arbitrary compiler warnings as AbsInt lint output.

### 7.3 Why CFG lints are crucial for “near-zero FP”

Certain classes of bugs are fundamentally path-sensitive:

- “unchecked division” requires proving a zero-check dominates all division sites.
- “destroy_zero requires balance==0” requires tracking that the value is proven zero on all relevant paths.

CFG lints can be designed to have near-zero false positives if they:

- model only what they can prove (do not guess),
- use conservative joining and do not “assume validated” in ambiguous joins,
- keep the warning condition as a “proof obligation not met”.

This directly aligns with the tiering concept: CFG lints can often be preview-tier initially for performance/edge cases, then promoted once validated.

---

## 8. Cross-Module Analysis (Phase III)

### 8.1 Where it lives

Cross-module analysis exists in `src/cross_module_lints.rs`.

It builds call graphs and performs analyses such as:

- transitive capability leak detection,
- flashloan/hot-potato repayment reasoning (conservative).

### 8.2 Key challenges

Cross-module lints are the hardest to make both:

- fast enough for routine use, and
- precise enough for stable-tier guarantees.

Typical pitfalls:

- incomplete modeling of dynamic dispatch/overload patterns,
- not distinguishing root package vs dependencies,
- summarizing callees too coarsely (leading to false positives).

Given those realities, cross-module lints are appropriately tiered as experimental in descriptors, and should typically stay opt-in until performance and precision are proven.

---

## 9. Unified Registry (Cross-phase metadata view)

### 9.1 What it is

`src/unified.rs` implements a metadata registry that can register descriptors from:

- Phase I syntactic rules,
- Phase II semantic descriptors (in `src/semantic.rs`),
- Phase II AbsInt descriptors (`src/absint_lints.rs`),
- Phase III cross-module descriptors (`src/cross_module_lints.rs`).

### 9.2 Why it matters

This is the path toward eliminating multiple competing “lint lists” and hard-coded gating maps:

- Historically, Phase I used multiple sources of truth (name lists and name→tier maps) in addition to per-lint descriptors.
- That duplication has since been removed: the unified registry and per-phase descriptor sources are now the authoritative inventory for CLI output and tier gating.

A unified registry can become the single “truth” for:

- tiers,
- analysis kinds,
- categories,
- required mode flags,
- and documentation/`list-rules` output.

If you want “zero FP stable tier”, it is extremely valuable to ensure “tier” is never duplicated or hand-mirrored in multiple places.

---

## 10. Lint Development Workflow (How to add lints)

### 10.1 Fast (tree-sitter) lint steps

1. Add lint implementation to a relevant file under `src/rules/`.
2. Define `static LintDescriptor` with:
   - correct tier (`RuleGroup`),
   - correct `AnalysisKind::Syntactic`,
   - correct fix metadata.
3. Export the lint type in `src/rules.rs`.
4. Register it in the Phase I registry builder in `src/unified.rs` (`build_syntactic_registry()`).
5. Add tests:
   - unit tests close to the lint, and/or
   - golden test fixtures, and/or
   - FP prevention tests.

### 10.2 Semantic / CFG / Cross-module lint steps

Semantic lints require deciding the correct integration point:

- **Type-based (typing AST / typing info):** implemented directly in `src/semantic.rs` and run inside `lint_package`.
- **CFG / AbsInt:** implement a new visitor in `src/absint_lints.rs`, register it via `create_visitors`, and create a diagnostic mapping (info code → descriptor).
- **Cross-module:** add new analysis functions and descriptors to `src/cross_module_lints.rs`, then invoke them from `src/semantic.rs`.

### 10.3 Most important development rule for “zero FP”

If a stable-tier lint cannot be phrased as:

- a type-system proven fact,
- an exact API misuse with exact identity checks,
- or a CFG proven obligation failure,

then it should not be stable-tier.

In other words: **stable-tier must be proof-oriented**.

---

## 11. Suppressions and Annotations (Current vs intended)

### 11.1 Current suppression mechanism

Fast-mode diagnostics use `suppression::is_suppressed_at` (`src/suppression.rs`) to honor:

- `#[allow(lint::name)]` on an enclosing item (module/function/struct/etc.)

The suppression logic:

- finds an “anchor item start” byte,
- scans backward a limited window for the exact `#[allow(lint::name)]`.

This is simple and deterministic, but limited:

- (previously) no `deny` / `expect`,
- no “allow by category” (`lint::security`, `lint::style`),
- no scoping beyond the immediate anchor item,
- no structured attribute parsing.

This has since been strengthened for fast-mode lints:

- `allow`/`deny`/`expect` directives (exact lint names) are now applied consistently at:
  - module/file scope via `#![...]`, and
  - item scope via `#[...]`.
- Directives also support category-level controls using `lint::<category>` (e.g. `lint::style`, `lint::modernization`), matching against `LintDescriptor.category`.
- The engine pre-collects item directives up front so `#[expect(...)]` is enforced even if the scope produces zero findings.
- `#[expect(lint::...)]` / `#![expect(lint::...)]` are enforced: if the lint does not fire in the declared scope, move-clippy emits an `unfulfilled_expectation` error diagnostic.

### 11.2 Intended richer system (partially integrated)

`src/annotations.rs` defines parsing for:

- `allow`, `deny`, `expect`,
- `validates(param)` (semantic meaning),
- and includes an explicit `SuppressionStack`.

This module is now wired into the fast lint engine for directive handling:

- `LintContext` applies module- and item-level directives using `annotations::module_scope(...)` and `annotations::item_scope(...)`.
- The engine pre-collects item directives up front so `#[expect(...)]` is enforced even if the scope produces zero findings.

Remaining gap:

- `validates(param)` is parsed but not yet used to power semantic rules.

### 11.3 Why this matters for “zero FP”

Suppressions are a key component of lint trust:

- If stable lints are truly low-noise, suppressions are rare.
- When suppressions are needed, being able to suppress by category/tier can prevent boilerplate.
- `expect` is a powerful correctness tool: it makes tests self-checking (“this lint should fire here”).

Integrating `annotations.rs` would strengthen both developer ergonomics and testing rigor.

---

## 12. Testing Architecture (What exists, what’s missing)

### 12.1 What exists today

Under `tests/`, the repository includes multiple layers:

- **Basic lint tests:** e.g., `tests/lints.rs`
- **Golden tests:** `tests/golden_tests.rs` with fixture directories under `tests/golden/<lint>/`
- **Fix tests:** `tests/fix_tests.rs`
- **Ecosystem baseline tests:** `tests/ecosystem.rs` and `tests/baselines/*.json`
- **Semantic integration sanity test:** `tests/semantic.rs` calls `semantic::lint_package` (feature-gated)
- **Semantic package snapshot tests:** `tests/semantic_package_snapshots.rs` calls `semantic::lint_package` and snapshots its output (feature-gated)
- **Phase II/III integration tests:** `tests/phase2_phase3_integration.rs` ensures visitors and descriptors exist
- **Spec-driven tests:** `tests/spec_tests.rs` includes exhaustive tests for some lints

### 12.2 Gaps / mismatches

Previously, there was a naming mismatch: `tests/semantic_snapshots.rs` was named and described as “Semantic Lint Snapshot Tests”, but it actually ran the **tree-sitter** pipeline:

- `LintEngine::lint_source` (syntactic-only)

That meant it could not validate:

- type-based lints,
- AbsInt lints,
- cross-module lints,
- or Sui delegated lints.

This has been corrected in-repo:

- The tree-sitter snapshot test has been renamed to `tests/syntactic_snapshots.rs` (to match what it actually covers).
- A feature-gated semantic harness now exists at `tests/semantic_package_snapshots.rs`, which exercises `semantic::lint_package` on fixture packages.

Related hardening:

- The parser now masks module-level `#![allow(lint::...)]` / `#![deny(lint::...)]` / `#![expect(lint::...)]` directives before parsing so tree-sitter does not emit `ERROR` nodes for those lines, while preserving byte offsets.

### 12.3 Why ecosystem baselines are necessary but not sufficient

Ecosystem baselines (e.g., OpenZeppelin Sui, DeepBook) are excellent regression guards, but:

- they can mask false negatives (missing detections) unless you actively maintain “known positives” too,
- and they can’t mathematically guarantee “zero FP” unless you treat them as exhaustive (they never are).

They are best used alongside spec-driven and mutation-driven suites.

---

## 13. “Zero False Positives” Methodology

“Zero false positives” is primarily a **design constraint**, not a post-hoc test property.

### 13.1 Stable-tier rule: only warn on proven conditions

For stable-tier lints:

- The lint must be phrased as: “I can prove this is wrong / suspicious by the compiler IR”.
- If the lint cannot be proven, it must not warn (accept false negatives).

Examples of “provable” evidence:

- ability sets (`key`, `store`, `copy`, `drop`) from typing info,
- exact callee identity (module/function resolved in typing AST),
- dominance/guard obligations in CFG,
- invariants that are purely structural and unambiguous.

### 13.2 Exhaustive finite-domain test matrices

When the property depends on a finite set of boolean capabilities (e.g., abilities), you can test all combinations:

- 2^N combinations, where N is number of abilities considered.

This is a powerful correctness argument:

- It provides very strong assurance that the lint will not fire in any non-triggering combination.

You already use this approach in `docs/FORMAL_LINT_SPECS.md` and `tests/spec_tests.rs`.

### 13.3 Mutation testing for CFG lints

CFG lints should be validated via:

1. Start from a “known correct” program that should not fire.
2. Create small mutations that break the obligation (remove guard, reorder, guard wrong var, guard only on one branch, etc.).
3. Require:
   - no warning on the original,
   - warning on each mutant.

This is particularly appropriate for:

- division-by-zero checks,
- `destroy_zero` obligations,
- “fresh address used once” constraints,
- dominance-based “validation before use” properties.

### 13.4 Differential testing vs Move Prover (future direction)

For certain lints, Move Prover specs could become a reference oracle:

- If prover says a property always holds but lint warns → false positive.
- If prover finds a counterexample but lint never warns → false negative.

This is a longer-term investment but aligns with “semantic testing” goals in a strong way.

---

## 14. Issues and Improvement Opportunities (Refactoring + Testing)

This section consolidates the highest-value improvements discovered while analyzing the codebase.

### 14.1 Single source of truth for tiers and lint metadata

**Observed issue (historical):** Lint metadata existed in multiple places:

- `LintDescriptor` defines group/analysis/category.
- Name lists (`FAST_LINT_NAMES`, `SEMANTIC_LINT_NAMES`) defined inventories.
- `get_lint_group(name)` separately mapped lint names to tiers.

**Why this is dangerous:**

- You can accidentally label a lint as `Experimental` in its descriptor but treat it as stable in another map or list.
- Tier gating becomes “best effort” rather than guaranteed.
- “zero FP stable tier” claims can silently degrade.

**Update:** This has been addressed via unified registry refactoring:

- The unified registry (`src/unified.rs`) is the canonical inventory for CLI queries and tier gating.
- Per-phase descriptor sources are authoritative; duplicated tier/name maps were removed.

### 14.2 Deprecated-tier gating consistency

**Update:** Deprecated-tier opt-in behavior was unified so `Deprecated` is treated like `Experimental` for gating and CLI exposure, and lint inventories are derived from descriptors.

### 14.3 Preview vs Experimental: unify the “flag story”

Today:

- CLI has `--preview` and `--experimental`.
- config has only `preview`.
- full-mode API takes only `preview: bool` and treats experimental as “preview=true for now”.

**Why this matters:**

- If you want a strict “Stable-only” run before ecosystem testing, you need consistent behavior.
- If you want to run experimental research rules, you need explicit “double opt-in” and consistent labeling.

**Recommended fix direction:**

- Add `experimental: bool` to config.
- Propagate both flags into:
  - registry filtering,
  - `semantic::lint_package`,
  - cross-module and AbsInt gating.
- Ensure “preview implies stable+preview; experimental implies stable+preview+experimental”.

### 14.4 Suppression / annotations unification

**Observed issue:** Two parallel systems exist:

- `src/suppression.rs` is used by `LintContext`.
- `src/annotations.rs` defines richer behavior but is unused.

**Recommended fix direction:**

- Decide to either:
  - remove `annotations.rs` if it’s not intended, or
  - integrate it as the unified suppression/annotation handler.

If integrated, it can unlock:

- module-level `#![allow(lint::security)]`,
- `deny` promotion,
- `expect` assertions (useful for tests),
- and domain-specific semantic hints like `#[validates(param)]`.

### 14.5 Reduce false positives by structural parsing (fast mode)

**Observed pattern:** Some lints match by scanning node text and checking `contains(...)`.

This is fragile because it can match:

- comments,
- string literals,
- unrelated identifiers containing similar substrings,
- nested contexts not actually calls.

**Recommended fix direction:**

- Prefer extracting callee identifiers via tree-sitter node fields.
- If necessary, build small utility functions:
  - “extract fully qualified call path”,
  - “extract function identifier token”,
  - “ignore comments/strings”.

This is a direct quality upgrade for fast-mode lints and reduces reliance on “heuristics”.

### 14.6 Test suite correctness: semantic tests must use semantic pipeline

Rename or rework “semantic snapshots” that currently run tree-sitter.

**Recommended fix direction:**

- Create fixture Move packages for semantic tests (already present in parts).
- Snapshot the results of `semantic::lint_package` for:
  - type-based lints,
  - AbsInt lints,
  - cross-module lints,
  - Sui delegated lints.

### 14.7 Add meta-tests that enforce quality invariants

The docs mention lint-quality meta-tests (and there’s existing quality assessment documentation). Consider implementing systematic invariants as tests, such as:

- every stable lint has:
  - at least one negative “no fire” test,
  - and at least one positive test,
  - and a description above a minimum length,
  - and no substring matching on raw node text unless explicitly justified.
- every security lint has a “source citation” convention.
- all descriptors are reachable from the registry (no orphaned descriptors).

Meta-tests are a low-effort, high-leverage way to keep quality high as the lint set grows.

---

## 15. Recommended Roadmap (Practical sequencing)

This roadmap is designed to improve correctness and maintainability first, without requiring ecosystem runs yet.

### Phase A (Foundational correctness): unify registry + gating

1. Make `LintDescriptor` + unified registry the source of truth.
2. Remove duplicated tier maps and list divergence.
3. Make preview/experimental/deprecated gating consistent across:
   - CLI (`src/cli.rs`),
   - config (`src/config.rs`),
   - fast mode registry filtering (`src/lint.rs`),
   - full mode filtering (`src/semantic.rs`).

**Outcome:** You can confidently say “stable-only means stable-only” across all paths.

### Phase B (Testing correctness): ensure “semantic” tests are semantic

1. Rename syntactic snapshots and add true semantic package snapshots:
   - `tests/syntactic_snapshots.rs` (tree-sitter)
   - `tests/semantic_package_snapshots.rs` (compiler-based; `--features full`)
2. Add mutation-style suites for CFG lints.
3. Expand spec-driven test matrices where applicable.

**Outcome:** You can validate “zero FP by design” properties locally without needing external repos.

### Phase C (Precision improvements): reduce heuristic reliance in fast mode

1. Replace raw-text substring matching with structural extraction helpers.
2. Strengthen negative tests (FP prevention).

**Outcome:** stable-tier fast lints become much more trustworthy and easier to maintain.

### Phase D (Ecosystem readiness): baselines and promotion criteria

1. Run ecosystem baselines for stable-tier only.
2. Promote preview lints only once:
   - they have adequate spec/mutation coverage,
   - and ecosystem baselines show zero FPs over time.

**Outcome:** stable-tier remains high-trust, while preview/experimental remain explicitly opt-in.

---

## 16. Status Update: Completed “Big Sweep” (Suppression + Parser Masking + Snapshots)

These were completed to make fast-mode lint behavior more trustworthy before any ecosystem runs.

### 16.1 Bulletproof suppression for fast lints

Goal: eliminate suppression bypass paths in fast-mode lints.

Completed changes:

- Added suppression-aware helpers on `LintContext` to report diagnostics anchored to a node/span.
- Converted fast-mode lint call sites away from `ctx.report_diagnostic(...)` to suppression-aware reporting.
- Confirmed module-level `#![allow(lint::...)]` reliably suppresses lints inside functions.
- Added a regression meta-test that fails CI if `ctx.report_diagnostic(` reappears under `src/rules/` (prevents suppression bypass from creeping back).

Notable consequence:

- Some snapshot expectations changed because allow directives now work consistently.

### 16.2 Mask `#![...]` directives before tree-sitter parse

Goal: avoid tree-sitter producing `ERROR` nodes for `#![allow(lint::...)]` and prevent lints from misbehaving around that parse artifact.

Completed changes:

- Added a pre-parse masking pass that replaces directive text with same-length whitespace (byte offsets/spans preserved).
- Added parser unit tests covering `#![allow(...)]`, `#![deny(...)]`, and `#![expect(...)]`, plus formatting edge cases (indentation; no trailing newline).

### 16.3 Clarify “semantic snapshot” naming; add real semantic snapshots

Goal: avoid false confidence from syntactic tests being labeled semantic.

Completed changes:

- Renamed the syntactic snapshot test file to `tests/syntactic_snapshots.rs`.
- Added `tests/semantic_package_snapshots.rs` (feature-gated) that snapshots real `semantic::lint_package` output on fixture packages.

How to run the semantic package snapshots:

- `cargo test --features full --test semantic_package_snapshots`
- Update snapshots: `INSTA_UPDATE=always cargo test --features full --test semantic_package_snapshots`

### 16.4 Integrate `allow`/`deny`/`expect` directives (fast mode)

Goal: replace ad-hoc allow-only suppression with a unified directive system that supports `deny` and testable `expect`.

Completed changes:

- Centralized directive parsing in `src/annotations.rs` (separate module-level `#![...]` scanning vs item-level `#[...]`).
- Applied directives consistently in `LintContext` so fast-mode diagnostics respect `allow`/`deny` and `expect`.
- Enforced `expect` by emitting an `unfulfilled_expectation` error when an expected lint does not fire.
- Added category-level directive support (`lint::style`, `lint::modernization`, etc.) with integration tests in `tests/expect_deny_integration.rs`.

---

## Appendix: Key Code Locations (Quick Index)

- Engine entry: `src/lib.rs`
- CLI: `src/cli.rs`
- Fast lint framework + syntactic registry wiring: `src/lint.rs`, `src/unified.rs`
- Fast lint implementations: `src/rules/*`
- Full-mode compiler integration + Sui lint delegation: `src/semantic.rs`
- CFG/AbsInt lints: `src/absint_lints.rs`
- Cross-module lints: `src/cross_module_lints.rs`
- Unified registry: `src/unified.rs`
- Suppression (currently used): `src/suppression.rs`
- Directive parsing (ext/legacy + masking support): `src/annotations.rs`, `src/parser.rs`
- Testing:
  - Golden tests: `tests/golden_tests.rs`
  - Ecosystem baselines: `tests/ecosystem.rs`, `tests/baselines/`
  - Semantic integration sanity: `tests/semantic.rs`
  - Spec-driven tests: `tests/spec_tests.rs`
  - Syntactic snapshots (tree-sitter): `tests/syntactic_snapshots.rs`
  - Semantic package snapshots (compiler-based; `--features full`): `tests/semantic_package_snapshots.rs`
  - Full-mode directives: `tests/semantic_directives_full_mode.rs`

---

## Cross-Chain Audit Themes → Move/Sui Lint Opportunities (Research Synthesis)

See also: `docs/notes/GPT-5.2-Audit-Research-Cross-Chain.md` for a more exhaustive, link-heavy research appendix.

This section answers a different question than the earlier “framework refactor” sections:

1. What do auditors *actually* keep finding across ecosystems (EVM, Solana, Move/Sui)?
2. Which of those classes are still plausible on Move/Sui?
3. Which of those are amenable to **strong, low/zero-false-positive lints** (and which are inherently “design review”)?

The punchline is:

- The *highest-impact* universal problems are still **authorization**, **validation**, and **economic correctness**.
- Move/Sui eliminates whole classes (notably “untrusted external call reentrancy”), but **does not** eliminate business-logic, oracle, access control, rounding, and DoS hazards.
- Most “universal” issues do not become Stable-tier lints immediately; they become **spec-driven fixtures** + **Preview-tier checks** with hard, explicit preconditions.

### Primary sources (selected)

These are the concrete references used to ground the taxonomy below. They’re intentionally a mix of (a) audit meta-summaries, (b) chain-specific “common pitfalls” posts, and (c) Move/Sui-specific audit primers.

- Trail of Bits: “246 Findings From our Smart Contract Audits: An Executive Summary” (audit meta-summary)
  - https://blog.trailofbits.com/2019/08/08/246-findings-from-our-smart-contract-audits-an-executive-summary/
- OWASP Smart Contract Top 10 (2023 + 2025 lists)
  - https://owasp.org/www-project-smart-contract-top-10/
- OpenZeppelin: “Web3 Security Auditor’s 2024 Rewind” (incident + audit pattern recap)
  - https://www.openzeppelin.com/news/web3-security-auditors-2024-rewind
- Zellic: “Top 10 Most Common Bugs In Your Aptos Move Contract” (Move audit pattern recap)
  - https://www.zellic.io/blog/top-10-aptos-move-bugs
- SlowMist: “Introduction to Auditing Sui — Move Contracts” + the associated GitHub primer/checklist
  - https://slowmist.medium.com/slowmist-introduction-to-auditing-sui-move-contracts-da005149f6bc
  - https://github.com/slowmist/Sui-MOVE-Smart-Contract-Auditing-Primer
- Neodyme: “Solana Smart Contracts: Common Pitfalls and How to Avoid Them” (SVM audit pitfalls)
  - https://neodyme.io/en/blog/solana_common_pitfalls
- Asymmetric Research: “Invocation Security: Navigating Vulnerabilities in Solana CPIs” (CPI-specific pitfalls)
  - https://blog.asymmetric.re/invocation-security-navigating-vulnerabilities-in-solana-cpis/
- Trail of Bits: Bluefin Sui Move security assessment (example Sui audit report with concrete Move-centric findings)
  - https://bluefin.io/blog/doc/bluefin_sui_final_report.pdf

### Universal taxonomy (what keeps showing up everywhere)

Below is the most stable cross-chain taxonomy I’ve seen hold up across audits and public postmortems, regardless of VM:

1. **Authorization / access control mistakes**
   - Missing checks (anyone can call something meant to be privileged)
   - Checks applied in one code path but missing in another (inconsistent enforcement)
   - Confused-deputy / capability forwarding
   - Centralization risk (admin can do too much; key compromise = total loss)

2. **Data / input validation failures**
   - Parameters not range-checked
   - Address/object/account identity not validated
   - Missing freshness checks (timestamps, oracle rounds)
   - Wrong program/package/module IDs accepted (dependency confusion)

3. **Arithmetic & precision / rounding hazards**
   - Rounding-to-zero (fees become 0)
   - Order-of-operations mistakes (divide too early; “precision loss before multiplication”)
   - Boundary conditions (underflow/overflow, division-by-zero, saturating assumptions)

4. **Economic / oracle manipulation**
   - Using manipulable spot prices (AMM ratio as oracle)
   - Flash-loan amplification (single-transaction large capital changes)
   - MEV / front-running / transaction reordering assumptions

5. **DoS / unbounded execution / resource exhaustion**
   - Iteration over attacker-controlled collections
   - Worst-case expensive paths reachable by unprivileged callers
   - Storage bloat or “must iterate over all users” anti-patterns

6. **External interaction hazards (the “call boundary” problems)**
   - EVM: reentrancy / unchecked external calls
   - SVM: CPI redirection, signer-forwarding, and account-owner/type confusions
   - Move/Sui: “dependency trust” (imported modules) and “authority forwarding” (capabilities)

7. **Initialization / state-machine correctness**
   - Missing initialization or re-initialization
   - Illegal state transitions
   - Invariants not preserved across entrypoints

8. **Randomness misuse**
   - Predictable sources (timestamps, block hashes)
   - Biased sampling
   - Inadequate commitment/reveal design

### Platform mapping: what changes between EVM vs Solana vs Move/Sui

Think of this as “same human mistakes, different manifestation.”

#### EVM (Solidity/Vyper)

Highest-profile: reentrancy, auth bugs, oracle manipulation, rounding/math, upgrade mistakes.

Many issues are caused by **dynamic external calls** (arbitrary contracts) + **shared mutable state**.

#### Solana (SVM)

A large fraction of severe bugs collapse to: **account validation mistakes**.

- Missing owner checks
- Missing signer checks
- Passing the wrong program account to a CPI (arbitrary CPI)
- PDA / seed / bump validation mistakes
- “Signer privilege forwarding” across CPIs

The conceptual analogue to EVM “untrusted external calls” is: **untrusted CPI target** + **untrusted account inputs**.

#### Move (Aptos/Sui Move)

The language/bytecode verifier removes many “pointer-style” and “reentrancy-style” footguns.

What audits still find tends to be:

1. **Authority checks on shared objects / shared state**
2. **Capability containment (don’t leak the keys)**
3. **Generic type binding / type confusion**
4. **Arithmetic ordering/precision invariants**
5. **DoS / unbounded iteration reachable by untrusted callers**
6. **Dependency trust / “imported module is a boundary” assumptions**

The key is that “lintability” depends on whether we can (a) find a crisp syntactic/semantic signature and (b) justify a low false-positive precondition.

---

## Candidate lint shortlist (Move-Clippy oriented)

This is a *proposal backlog* (not implemented yet). Each item is framed in a Move-Clippy-native way:

- **Phase**: Fast (tree-sitter), Semantic (compiler), AbsInt (CFG), Cross-module
- **Tier recommendation**: Stable / Preview / Experimental
- **False-positive strategy**: the specific “hard preconditions” needed before we can plausibly ship as Stable

### 1) `shared_object_missing_authority_check` (Sui-specific)

Motivation: Shared objects are globally accessible. If an entry/public function mutates a `&mut <StructWithKeyAbility>` where the object is (or can become) shared, the function must implement an explicit authority model (capability check, allowlist, or sender verification).

Phase: Semantic (needs object type + ability info + call graph hints)

Tier: Preview initially

Hard preconditions (to reduce FPs):

- Only trigger when:
  - the function is `public entry` (or `public`) and
  - takes `&mut <StructWithKeyAbility>` and
  - does *not* take a `&signer` and does *not* take any parameter whose type name matches a configured “capability allowlist” (e.g., `*Cap`, `*Capability`, `*Authority`), and
  - the function body performs at least one state-mutating operation on that object (writes to a field, moves assets, emits events tied to state changes).

Why not Stable immediately: There exist legitimate patterns where authority is enforced indirectly (e.g., object is address-owned, not shared; or authority is validated via a wrapper object).

How to push toward Stable: Add a Move-model-aware check for “is this object actually shared?” which may require whole-package reasoning or heuristics.

### 2) `capability_leak_via_return_or_transfer` (Move/Sui)

Motivation: Many Move designs rely on a “capability” (admin cap, mint cap, treasury cap) that must not leak to untrusted callers.

Phase: Fast or Semantic (fast can start with name-based heuristic; semantic can use ability info)

Tier: Preview

Hard preconditions:

- Only trigger when an `entry` function:
  - returns a value whose type name matches a configured capability allowlist/pattern, OR
  - calls `transfer::public_transfer`, `transfer::transfer`, or `transfer::share_object` on such a capability value.

Zero-FP angle: Make the lint configurable and off-by-default for generic “*Cap” heuristics; ship Stable only for known dangerous concrete types (e.g., `TreasuryCap<T>` transfers) if the ecosystem agrees.

### 3) `generic_type_not_bound_to_state` (Aptos Move theme; often relevant to Sui too)

Motivation: Zellic calls out lack of generics type checking/type confusion as a common Move bug class: the function is generic over `T`, but the state being accessed/stored is for some specific token/type, and the code never checks that `T` matches that stored type.

Phase: Semantic (must inspect type parameters and stored “type tags” patterns)

Tier: Experimental (highly pattern-dependent)

Hard preconditions:

- Only trigger when the function is generic over `T` *and* reads a stored “type witness” value (e.g., `TypeName`, `type_name::get<T>()`, or a stored `TypeTag`) *and* does not compare it to the generic parameter.

Note: This lint is best implemented as a suite of narrow pattern lints rather than one mega-lint.

### 4) `unbounded_iteration_over_user_input` (DoS/gas)

Motivation: Unbounded loops are a top Move audit finding (Zellic: “Unbounded Execution”).

Phase: AbsInt (CFG-based path reasoning; avoid trivial syntactic false positives)

Tier: Preview

Hard preconditions:

- Trigger only when:
  - a loop bound is derived from `vector::length(v)` where `v` is a *parameter* (not a local), and
  - the loop performs at least one “costly” operation (nested loops, table access, object transfers), and
  - the function is `public entry`.

Why not Stable: Transaction size limits and economic constraints can make some unbounded loops acceptable; it’s protocol-dependent.

### 5) `division_before_multiplication_precision_loss` (math ordering)

Motivation: Both EVM and Move audits repeatedly find “division too early” issues.

Phase: Fast or Semantic

Tier: Experimental unless paired with a domain-specific invariant.

Hard preconditions for Stable:

- Needs a very narrow invariant, e.g. code computes a fee as `(amount / denom) * fee_bps` where `fee_bps < denom` and then asserts fee > 0 elsewhere (or relies on it).
- Without a domain invariant, this remains a helpful “math smell” but not zero-FP.

### 6) `missing_coin_registration_check` (Aptos-specific; Sui variant differs)

Motivation: On Aptos, failing to ensure account registration for coin types is a recurring issue.

Sui relevance: Sui’s coin model differs (objects), so a direct port likely doesn’t make sense, but the *concept* (“precondition on asset account/object state”) does.

Phase: Probably out-of-scope unless Move-Clippy explicitly supports Aptos packages; listed here primarily as a reminder that “asset preconditions” are a recurring audit theme.

### 7) `unchecked_return_value_of_critical_call` (Move/Sui)

Motivation: SlowMist explicitly calls out unchecked return values as an audit focus.

Phase: Semantic (needs type info) or Fast (heuristic allowlist of functions)

Tier: Preview

Hard preconditions:

- Only trigger for a curated allowlist of functions whose return is semantically a success/failure indicator (or a value that must be used).
- Optionally require the call occurs inside an `entry` function.

### Proposal overlap check (what’s already implemented vs new)

This “candidate shortlist” was written intentionally as proposals, but some of the *ideas* already exist in Move-Clippy today under different lint names.

**How to verify duplicates (authoritative):**

- The authoritative inventory is `docs/LINT_REFERENCE.md` (generated from the unified registry).
- At runtime, the authoritative set in *your build* is what `move-clippy list-rules` prints.
- Use `move-clippy explain <lint>` to confirm semantics before proposing a new rule name.

**Important nuance:** docs like `docs/SECURITY_LINTS.md` are helpful narrative, but they can drift; if a lint name is not present in `docs/LINT_REFERENCE.md`, it is not currently registered/available in the binary.

#### Mapping: proposed idea → existing lint(s)

1. **`shared_object_missing_authority_check`**
   - **Closest existing coverage:**
     - `shared_with_balance` (Preview, semantic): shared object contains a `Balance` field → likely needs access control.
     - `share_owned_authority` (Stable, semantic): sharing key+store object is dangerous for authority objects.
     - `unused_capability_param` (Stable, semantic): capability is passed but not used (often indicates missing check).
   - **What’s still missing vs the proposal:**
     - A general “public entry mutates shared state without explicit authority model” lint with hard preconditions (and ideally, “is this object actually shared?” reasoning).

2. **`capability_leak_via_return_or_transfer`**
   - **Already covered in spirit (transfer/leak axis):**
     - `capability_transfer_v2` (Experimental, semantic): capability transferred to non-sender address (type-based).
     - `transitive_capability_leak` (Experimental, cross-module): capability leaks across module boundary.
     - `store_capability` (Preview, semantic): capability-like struct has `store` (dynamic-field leak risk).
     - `copyable_capability` (Preview, semantic): capability-like struct has `copy` (duplication breaks auth).
     - `capability_destroyed_unused` / `phantom_capability` (Preview/Experimental, absint): capability consumed/unused/not validated patterns.
   - **What’s still missing vs the proposal:**
     - A narrowly-scoped “entry function returns a capability-like value” or “entry transfers/shares a capability-like value” lint that is intentionally tuned for near-zero FPs via an allowlist (rather than broader capability heuristics).

3. **`generic_type_not_bound_to_state`**
   - **Status:** Not currently implemented (no generics/type-confusion lint appears in `docs/LINT_REFERENCE.md`).
   - **Related existing themes (but not the same):** some lints reason about “type-based” semantics, but none enforce “generic parameter must match stored witness.”
4. **`unbounded_iteration_over_user_input`**
   - **Status:** Not implemented as proposed.
   - **Closest existing coverage:**
     - `unbounded_vector_growth` (Preview, syntactic): flags likely-unbounded growth patterns (vector push/extend in patterns that can lead to DoS/storage blowup).
   - **What’s still missing vs the proposal:**
     - A CFG-aware (AbsInt) “loop bound derived from user input” lint with cost heuristics.

5. **`division_before_multiplication_precision_loss`**
   - **Status:** Not implemented as proposed.
   - **Closest existing coverage:**
     - `unchecked_division_v2` (Preview, absint): division without zero-check.
     - `unsafe_arithmetic` (Experimental, syntactic): broad arithmetic smell detection.
   - **What’s still missing vs the proposal:**
     - A precision/rounding-order lint that can be proven low-FP (likely requires domain-specific patterns like fee math).

6. **`missing_coin_registration_check`**
   - **Status:** Not implemented.
   - **Note:** This is Aptos-specific; Sui’s coin model is object-centric, so a direct port may not be appropriate.

7. **`unchecked_return_value_of_critical_call`**
   - **Already implemented (essentially):**
     - `unused_return_value` (Stable, semantic): important return value ignored.
   - **Adjacent (often confused with this):**
     - `entry_function_returns_value` (Stable, semantic): entry returns are discarded by runtime (zero-FP correctness lint).

---

## How to turn audit themes into *zero-FP* Stable lints

This is the Move-Clippy-specific “bridge” between broad audit categories and shipping Stable-tier lints.

1. **Start from a concrete bug pattern**
   - Prefer patterns with a single “smoking gun” signature (e.g., a sensitive entrypoint that returns/transfers a capability type).

2. **Write the lint contract as tests-first fixtures**
   - Positive fixture: minimal code that triggers
   - Negative fixture: near-miss that must not trigger
   - Directive fixture: `allow/deny/expect` coverage (especially `expect` to freeze intent)

3. **Pick the phase that can express the preconditions**
   - If a lint needs type/ability info, it’s semantic/absint/cross-module, not fast.
   - If a lint is fundamentally “naming convention smell,” keep it Preview/Experimental.

4. **Treat Stable promotion as an evidence-gated step**
   - Evidence = zero unexpected hits on fixtures + curated ecosystem sample
   - If it fires unexpectedly, either tighten preconditions or keep it Preview.

5. **Exploit Move’s strengths**
   - Don’t waste Stable-tier bandwidth linting what the compiler/verifier already proves.
   - Focus on “human intent gaps”: authority, invariants, and economic assumptions.
