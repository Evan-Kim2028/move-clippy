# Rule Stability Policy

Move Clippy uses a stability classification system inspired by [Ruff](https://docs.astral.sh/ruff/) to keep default linting low-noise and production-safe.

**Status:** Policy (kept current)

**Authoritative catalog:** Do not maintain “current lint lists” by hand. For the up-to-date set of lints (tiers, phases, and requirements), use:

- `docs/LINT_REFERENCE.md` (generated; see header for regen command)
- `move-clippy list-rules` (authoritative for your build + features)

---

## Analysis Types (How a lint reasons)

Move Clippy uses different analysis techniques with different cost/precision trade-offs:

| Analysis Type | Mode Required | What it can prove |
|--------------|---------------|-------------------|
| **Syntactic** | `--mode fast` | Pure pattern/structure facts from the parse tree |
| **TypeBased** | `--mode full` | Type/ability facts grounded in compiler typing info |
| **TypeBasedCFG** | `--mode full --preview` | Value-flow facts that must hold across control-flow (dominance, joins) |
| **CrossModule** | `--mode full --experimental` | Whole-program relationships (call graph, transitive flows) |

**Policy implication:** “Tier” is about *expected false-positive rate* and *stability*, not about the category (security/style). A security lint can be Stable if it is type-grounded and precise; a style lint can be Experimental if it relies on brittle heuristics.

---

## Rule Groups (Tiers)

### Stable

**Definition:** Rules enabled by default. Intended to be safe for CI.

**Quality bar:**
- Extremely low false-positive rate in practice
- Clear messages that explain the invariant being enforced
- Suppression works reliably at module + item scope
- If a fix is provided, it is either Safe (default) or explicitly marked Unsafe

**Required testing:**
- Positive + negative fixtures
- Suppression test (allow/deny/expect)
- Snapshot coverage or spec test coverage appropriate to the analysis kind

### Preview

**Definition:** Rules that are intended to be high precision but are still validating ergonomics/performance and edge cases.

**Typical reasons to stay in Preview:**
- The invariant is correct, but the anchor/scope behavior is still being tuned
- The implementation is precise but expensive; needs perf validation
- The rule is new and needs ecosystem validation before becoming default

**Required testing:**
- Same as Stable, plus at least one “edge formatting / edge structure” fixture

### Experimental

**Definition:** Rules intended for audits/research, where more noise is acceptable.

**Typical characteristics:**
- Heuristic detection (naming patterns, best-effort inference)
- Cross-module analysis with known edge cases
- Higher computational cost

**Required testing:**
- At minimum, a fixture that demonstrates the intended finding and a fixture that demonstrates a near-miss negative
- If suppression is supported, a suppression fixture

### Deprecated

**Definition:** Rules that exist for compatibility but should not be used going forward.

**Typical reasons:**
- Superseded by a more precise `_v2` (often CFG-based)
- The underlying behavior is enforced by the runtime/compiler
- The rule was found to be too noisy or unmaintainable

---

## Tier Promotion Criteria

A lint is promoted when it demonstrates:

- **Correctness:** The invariant matches real bugs and does not misclassify common legitimate patterns.
- **Precision:** False positives are rare across representative packages.
- **Ergonomics:** Messages are actionable and suppression works predictably.
- **Stability:** The behavior is unlikely to churn due to parser/compiler implementation details.

---

## “Zero False Positives” Methodology

Stable/Preview lints should be designed so they only fire when the tool can justify the finding with strong evidence.

Preferred approaches (in order):

1. **Type-grounded detection** (abilities, key/store, concrete framework APIs)
2. **CFG/dataflow proofs** (dominance, taint tracking, path coverage)
3. **Cross-module proofs** only when the call graph is reliable and the invariant is well-scoped
4. **Heuristics** only for Experimental (or for “suggestion-only” diagnostics)

**Testing discipline:** Treat `tests/fixtures/` as executable documentation. Every lint should have a minimal contract fixture that shows:
- a true positive,
- a true negative,
- suppression/expect behavior.
