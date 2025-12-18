# GPT 5.2 — Cross-Chain Audit Research (EVM / Solana / Move) → Move-Clippy Lint Opportunities

**Workspace:** `learning_move/packages/move-clippy/`  
**Last updated:** 2025-12-18  
**Purpose:** Ground Move-Clippy lint ideation in real audit writeups and “common bug” summaries across ecosystems, then translate what still applies to Sui Move into **low-noise** lint candidates.

This note is intentionally verbose and link-heavy so it can be used as a durable “research appendix”.

---

## 1) Sources Used (Representative, Cross-Chain)

### EVM / general smart contract audit pattern summaries

- OWASP Smart Contract Top 10 (2025)
  - https://owasp.org/www-project-smart-contract-top-10/
  - Used for a high-level “universal vulnerability class” taxonomy (access control, oracle manipulation, reentrancy, integer/precision issues, etc).
- OpenZeppelin audit meta-posts
  - “What is a Smart Contract Audit: Lessons from OpenZeppelin’s audits”
    - https://www.openzeppelin.com/news/what-is-a-smart-contract-audit-lessons-from-openzeppelins-1000-audits
  - Used as a second opinion on “what auditors see repeatedly”.

### Solana (SVM) — common audit pitfalls and their root causes

- Neodyme: “Solana Smart Contracts: Common Pitfalls and How to Avoid Them”
  - https://neodyme.io/en/blog/solana_common_pitfalls
  - Core theme: *account validation mistakes* (owner/signer/data matching), and “arbitrary CPI” class issues.
- Helius: “A Hitchhiker’s Guide to Solana Program Security”
  - https://www.helius.dev/blog/a-hitchhikers-guide-to-solana-program-security
  - Emphasizes PDA bump canonicalization and account data matching/validation.
- Sec3: “Why You Should Always Validate PDA Bump Seeds”
  - https://www.sec3.dev/blog/pda-bump-seeds
  - A concrete example of how missing “canonical bump” validation becomes an exploit.
- Asymmetric Research: “Invocation Security: Navigating Vulnerabilities in Solana CPIs”
  - https://blog.asymmetric.re/invocation-security-navigating-vulnerabilities-in-solana-cpis/
  - A strong, modern writeup of the “arbitrary CPI target” + “signer privilege forwarding” class.

### Move / Sui Move — what audits repeatedly find

- Zellic: “Top 10 Most Common Bugs In Your Aptos Move Contract”
  - https://www.zellic.io/blog/top-10-aptos-move-bugs
  - Useful because it’s an explicit “audit-derived bug list” for Move.
- SlowMist: Sui Move smart contract auditing primer + checklist repo
  - https://github.com/slowmist/Sui-MOVE-Smart-Contract-Auditing-Primer
  - Strong for a Sui-specific audit checklist: object management, access control, arithmetic accuracy, race conditions, DoS, upgrade security.
- Mirage Audits: “The Ability Mistakes That Will Drain Your Sui Move Protocol”
  - https://www.mirageaudits.com/blog/sui-move-ability-security-mistakes
  - Sui-specific and highly relevant to Move-Clippy’s type-system-gap approach (abilities, capability objects, and shared-object hazards).
- MoveScanner (academic): “Analysis of Security Risks of Move Smart Contracts”
  - https://arxiv.org/html/2508.17964v2
  - A static analysis perspective on recurrent Move issues: resource leaks, weak permission management, unchecked return values, cross-module issues.

### Incident / postmortem signal (Sui ecosystem)

- Verichains: Cetus exploit analysis (math/overflow-check mistakes)
  - https://blog.verichains.io/p/cetus-protocol-hacked-analysis
  - Included because it’s a representative “math library bug → catastrophic economic loss” story.

---

## 2) What’s “Universal” Across EVM, Solana, Move?

Across ecosystems, auditors repeatedly find the same *human* failures, even if the VM/language makes the exploit mechanics different:

1. **Authorization mistakes**
   - privileged action reachable by an untrusted caller
   - “admin” checks that don’t actually bind to the caller you meant
   - authority objects (capabilities / admin roles) that leak to untrusted parties

2. **Validation failures**
   - missing “obligation checks” (bounds, non-zero, freshness, invariants)
   - relying on implicit preconditions that aren’t actually enforced on-chain
   - accepting untrusted inputs (addresses/IDs/keys) without verifying they’re tied to the signer/authority

3. **Economic correctness errors**
   - oracle manipulation / stale or single-source oracles
   - precision/rounding mistakes (order-of-ops, scaling errors)
   - accounting inconsistencies (“deposit 1, mint 1e18” style failures)

4. **Resource exhaustion / DoS**
   - unbounded iteration or worst-case expensive paths reachable by untrusted callers
   - abort-based DoS (making critical entrypoints abort under attacker-chosen inputs)

5. **Boundary / “call boundary” hazards**
   - EVM: reentrancy and unchecked external calls
   - Solana: untrusted accounts, untrusted CPI targets, signer forwarding
   - Move: cross-module trust assumptions and authority forwarding (capabilities)

Move changes the *shape* of these problems, but it doesn’t remove them.

---

## 3) What Move/Sui Prevents “By Construction” (and what it doesn’t)

### Strongly reduced vs EVM/SVM

- **Classic EVM-style reentrancy** is typically not the dominant class in Move audits (no dynamic fallback-style external call surface).
- **Accidental asset duplication** is harder because of Move’s resource model (linearity), but “ability mistakes” can reintroduce breakage.

### Still very much present

- **Business-logic authorization bugs** (who is allowed to do what) remain a top class.
- **Shared object hazards (Sui)**: converting the wrong object to shared, or mutating shared state without a clear authority model.
- **Capability leakage**: granting/returning/transferring authority objects in ways the design didn’t intend.
- **DoS via unbounded iteration**: attacker-controlled collections or parameters can still cause gas/abort denial.
- **Math/precision bugs**: fixed-point scaling and boundary checks remain subtle and dangerous.

---

## 4) Move-Clippy Coverage Map (What We Already Have)

This section maps “audit classes” to Move-Clippy lints that already exist, so we don’t accidentally propose duplicates.

To check the authoritative lint set in this checkout:

```bash
cargo run --features full --bin move-clippy -- list-rules
```

### Authorization / capability safety

- `share_owned_authority` (stable): flags sharing `key+store` objects via `transfer::share_object` / `public_share_object`.
- `shared_capability_object` (preview): flags sharing *capability-like* types (conservative type classifier; excludes coins).
- `capability_transfer_literal_address` (preview): flags transfers of capability-like objects to a literal address.
- `mut_key_param_missing_authority` (preview): flags `public entry` functions that take `&mut` key types but have no explicit authority param.
- `capability_transfer_v2` (experimental): flags capability transfers to non-sender addresses (more general; higher FP risk).
- `transitive_capability_leak` (experimental): cross-module flow of capabilities.

### Validation / correctness checks

- `unchecked_division_v2` (preview, CFG): division without a dominating non-zero check.
- `destroy_zero_unchecked_v2` (preview, CFG): `destroy_zero` without a dominating “is zero” proof.
- `divide_by_zero_literal` (stable): literal `/ 0` or `% 0`.
- `unused_return_value` (stable): important return values ignored (explicit “unchecked return value” class).
- `unbounded_iteration_over_param_vector` (preview): loops bounded by `vector::length(param_vec)` in `public entry` context.

### Oracle / randomness / economic safety signals

- `stale_oracle_price` (stable): warns on unsafe price fetch patterns.
- `public_random_access` (stable): public exposure of randomness sources.
- `digest_as_randomness` (experimental): flags `tx_context::digest` used as randomness.
- `suspicious_overflow_check` (stable): flags error-prone manual overflow checks (motivated by incident patterns like math library bugs).

### Delegated Sui compiler lints (pass-through)

These run in `--mode full` and mirror Sui’s own lints under `move_compiler::sui_mode::linters`:

- `share_owned`, `self_transfer`, `custom_state_change`, `coin_field`, `freeze_wrapped`, `collection_equality`, `public_random`, `missing_key`, `freezing_capability`

---

## 5) How Many of the “Top Five” Proposals Are Already Implemented?

The following five audit-informed semantic lints are already implemented in this checkout (added as Preview/Experimental to stay low-risk before ecosystem validation):

- `shared_capability_object` (preview)
- `capability_transfer_literal_address` (preview)
- `mut_key_param_missing_authority` (preview)
- `unbounded_iteration_over_param_vector` (preview)
- `generic_type_witness_unused` (experimental)

These were chosen because they map cleanly to “universal” audit classes (authority leaks + unbounded execution + generic misuse) while still being expressible with the current semantic pipeline.

---

## 6) Next Lint Ideas (Promising, but Need Care to Avoid False Positives)

These are *not* implemented yet; they are proposed based on the sources above and the current engine capabilities.

### Candidate A: “Hardcoded admin address check”

**Problem signal:** Authorization logic that hardcodes a literal `@0x...` address, especially when compared to `tx_context::sender(ctx)` / `signer::address_of(...)`.

**Why it’s common:** Hardcoded admins appear frequently in audits as centralization risk and as a source of misconfiguration bugs (wrong address, testnet/mainnet mismatch).

**Why it’s hard:** It can be intentional and correct; the lint should likely be Preview/Experimental unless it proves “this is the only gate to a privileged action”.

### Candidate B: “Unchecked indexing / abort-based DoS”

**Problem signal:** `vector::borrow(_mut)?`, `table::borrow(_mut)?`, `dynamic_field::borrow(_mut)?` using attacker-controlled indices/keys without a guarding `contains`/bounds check.

**Why it matters on Move:** Even if it “only aborts”, aborting a critical entrypoint can be a denial-of-service vector.

**False-positive strategy:** Start narrow: only fire when the index/key is a direct function parameter and there is no local guard in the same block.

### Candidate C: “Shared object state mutation without an explicit authority pattern”

**Problem signal:** public entrypoint mutates a `&mut` value whose type is known/shared (or created as shared) without requiring a capability or signer and without checking sender.

**False-positive strategy:** Only fire when the function:
- is `public entry`,
- takes a `&mut` key type,
- and performs a state write on that object,
- and has no obvious authority input.

This is closely related to `mut_key_param_missing_authority`, but could be made more precise by proving actual mutation.

### Candidate D: “Capability returned from public function”

**Problem signal:** returning a capability-like type from a public entrypoint is often a direct authority leak.

**False-positive strategy:** narrow to “capability-like” types only and exclude coins; consider Preview first.

### Candidate E: “Friend/package overexposure” (Move-family generalization)

**Problem signal:** `public(friend)` / overly-broad `public(package)` on privileged helpers that were intended to be internal-only.

**Notes:** This is more Aptos Move than Sui Move (Sui uses package visibility patterns), but it’s directly called out in Move audit writeups as a common class.

---

## 7) Practical Workflow: From Audit Theme → New Lint

1. **Pick a class** from sources above (authorization, DoS, arithmetic precision, etc).
2. **State a crisp invariant** that can be checked locally (avoid “design review” lints as Stable).
3. **Choose a pipeline**:
   - Fast (tree-sitter) only when structure is enough.
   - Semantic when type/ability/program-info is required.
   - CFG (AbsInt) for “dominance” / “all paths” facts.
   - Cross-module only for carefully-scoped transitive flows.
4. **Start in Preview/Experimental** unless the invariant is obviously sound.
5. **Add fixtures as executable docs**:
   - positive, negative, suppression/expect.
6. **Promote only after validation**:
   - spec/mutation tests, then ecosystem baselines.

