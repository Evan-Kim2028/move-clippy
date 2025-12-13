# Security Lint Triage

**Last Updated:** 2025-12-13  
**Reviewer:** [Your Name]

---

## Summary

| Lint | Total | Confirmed | FP | WontFix | NeedsReview | FP% | Status |
|------|-------|-----------|----|---------|--------------|----|--------|
| droppable_hot_potato | - | - | - | - | - | - | ⏳ |
| excessive_token_abilities | - | - | - | - | - | - | ⏳ |
| shared_capability | - | - | - | - | - | - | ⏳ |
| stale_oracle_price | - | - | - | - | - | - | ⏳ |
| single_step_ownership_transfer | - | - | - | - | - | - | ⏳ |
| missing_witness_drop | - | - | - | - | - | - | ⏳ |
| public_random_access | - | - | - | - | - | - | ⏳ |

**Legend:**
- ✅ HIGH CONFIDENCE - 100% reviewed, FP < 5%
- ⚠️ MEDIUM CONFIDENCE - Partially reviewed, FP 5-25%
- ❌ LOW CONFIDENCE - High FP or unreviewed
- ⏳ PENDING - Not yet reviewed

---

## Detailed Analysis

### droppable_hot_potato

**Description:** Detects hot potato structs that incorrectly have the `drop` ability, allowing them to be silently discarded instead of being properly consumed.

**Audit Evidence:**
- Trail of Bits 2025: Hot potato pattern abuse in lending protocols
- Mirage 2025: Token duplication via drop ability

**Status:** ⏳ PENDING

**Findings:**

| ID | Repo | File:Line | Status | Notes |
|----|------|-----------|--------|-------|
| `abc123...` | alphalend | lp_position.move:116 | CONFIRMED | Real bug - fixed in commit 11d2241 |
| ... | ... | ... | ... | ... |

**Summary:**
- Total: X
- Confirmed: X
- False Positives: X
- FP Rate: X%

**Recommendation:** [Ready for Stable / Needs refinement / Keep in Preview]

---

### excessive_token_abilities

**Description:** Detects token/coin-like structs with both `copy` and `drop` abilities, which could allow token duplication.

**Audit Evidence:**
- Mirage 2025: Token ability misuse
- MoveBit 2023: Coin duplication vulnerabilities

**Status:** ⏳ PENDING

**Findings:**

| ID | Repo | File:Line | Status | Notes |
|----|------|-----------|--------|-------|
| ... | ... | ... | ... | ... |

**Summary:**
- Total: X
- Confirmed: X
- False Positives: X
- FP Rate: X%

**Recommendation:** [...]

---

### shared_capability

**Description:** Detects capability structs being shared publicly via `share_object`, which could allow unauthorized access.

**Audit Evidence:**
- OtterSec: Capability sharing vulnerabilities
- MoveBit 2023: Access control bypass via shared caps

**Status:** ⏳ PENDING

**Findings:**

| ID | Repo | File:Line | Status | Notes |
|----|------|-----------|--------|-------|
| ... | ... | ... | ... | ... |

**Summary:**
- Total: X
- Confirmed: X
- False Positives: X
- FP Rate: X%

**Recommendation:** [...]

---

### stale_oracle_price

**Description:** Detects usage of `get_price_unsafe()` or similar functions that return potentially stale oracle prices.

**Audit Evidence:**
- Bluefin Audit 2024: Stale price exploitation
- Multiple DeFi oracle manipulation incidents

**Status:** ⏳ PENDING

**Findings:**

| ID | Repo | File:Line | Status | Notes |
|----|------|-----------|--------|-------|
| ... | ... | ... | ... | ... |

**Summary:**
- Total: X
- Confirmed: X
- False Positives: X
- FP Rate: X%

**Recommendation:** [...]

---

### single_step_ownership_transfer

**Description:** Detects admin/owner transfers that happen in a single step without confirmation, risking permanent lockout.

**Audit Evidence:**
- Bluefin Audit 2024: Single-step admin transfer risks
- General best practice: Two-step ownership transfer

**Status:** ⏳ PENDING

**Findings:**

| ID | Repo | File:Line | Status | Notes |
|----|------|-----------|--------|-------|
| ... | ... | ... | ... | ... |

**Summary:**
- Total: X
- Confirmed: X
- False Positives: X
- FP Rate: X%

**Recommendation:** [...]

---

### missing_witness_drop

**Description:** Detects One-Time Witness (OTW) structs that are missing the `drop` ability.

**Status:** ⏳ PENDING

**Findings:**

| ID | Repo | File:Line | Status | Notes |
|----|------|-----------|--------|-------|
| ... | ... | ... | ... | ... |

**Summary:**
- Total: X
- Confirmed: X
- False Positives: X
- FP Rate: X%

**Recommendation:** [...]

---

### public_random_access

**Description:** Detects public functions that expose `Random` objects, enabling front-running attacks.

**Status:** ⏳ PENDING

**Findings:**

| ID | Repo | File:Line | Status | Notes |
|----|------|-----------|--------|-------|
| ... | ... | ... | ... | ... |

**Summary:**
- Total: X
- Confirmed: X
- False Positives: X
- FP Rate: X%

**Recommendation:** [...]

---

## Triage Commands

```bash
# List all security findings needing review
move-clippy triage list --category security --status needs_review

# Review specific lint
move-clippy triage list --lint droppable_hot_potato

# Update finding status
move-clippy triage update <id> --status confirmed --notes "Real bug, evidence: ..."

# Generate fresh report
move-clippy triage report --format md -o TRIAGE_REPORT.md
```

---

## Change Log

| Date | Reviewer | Changes |
|------|----------|---------|
| 2025-12-13 | - | Initial template created |
| ... | ... | ... |
