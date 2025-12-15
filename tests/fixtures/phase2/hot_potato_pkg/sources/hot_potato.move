// Test fixture for unused_hot_potato lint
// Tests CFG-aware hot potato consumption tracking

module hot_potato_pkg::hot_potato {

    // =========================================================================
    // Hot Potato Definition (no abilities = must be consumed)
    // =========================================================================

    // A hot potato struct - no abilities means it MUST be consumed
    public struct FlashLoanReceipt {
        pool_id: address,
        amount: u64,
    }

    // Empty witness type - should NOT be flagged (0 fields)
    public struct Witness {}

    // =========================================================================
    // TRUE POSITIVES - Should be flagged
    // =========================================================================

    // SHOULD WARN: Hot potato created but dropped at end of function
    public fun bad_unconsumed() {
        let _receipt = FlashLoanReceipt { pool_id: @0x1, amount: 100 };
        // Missing: repay(receipt) or return
        // The hot potato is silently dropped here - BUG!
    }

    // SHOULD WARN: Hot potato only consumed in one branch (else branch drops it)
    public fun bad_conditional(flag: bool) {
        let receipt = FlashLoanReceipt { pool_id: @0x1, amount: 100 };
        if (flag) {
            consume(receipt);
        };
        // else branch: receipt is dropped - BUG!
    }

    // =========================================================================
    // TRUE NEGATIVES - Should NOT be flagged
    // =========================================================================

    // SHOULD NOT WARN: Hot potato returned (caller's responsibility)
    public fun good_returned(): FlashLoanReceipt {
        FlashLoanReceipt { pool_id: @0x1, amount: 100 }
    }

    // SHOULD NOT WARN: Hot potato passed to consuming function
    public fun good_consumed() {
        let receipt = FlashLoanReceipt { pool_id: @0x1, amount: 100 };
        repay(receipt);
    }

    // SHOULD NOT WARN: Hot potato unpacked (destructured)
    public fun good_unpacked() {
        let receipt = FlashLoanReceipt { pool_id: @0x1, amount: 100 };
        let FlashLoanReceipt { pool_id: _, amount: _ } = receipt;
    }

    // SHOULD NOT WARN: Empty struct (witness type) - skipped by design
    public fun good_witness() {
        let _w = Witness {};
        // Empty structs are typically witness/marker types, not hot potatoes
    }

    // SHOULD NOT WARN: Hot potato consumed in both branches
    public fun good_conditional(flag: bool) {
        let receipt = FlashLoanReceipt { pool_id: @0x1, amount: 100 };
        if (flag) {
            consume(receipt);
        } else {
            repay(receipt);
        };
    }

    // SHOULD NOT WARN: Hot potato assigned then consumed
    public fun good_assigned() {
        let receipt = FlashLoanReceipt { pool_id: @0x1, amount: 100 };
        let r2 = receipt;
        repay(r2);
    }

    // =========================================================================
    // Helper functions (consume hot potatoes)
    // =========================================================================

    fun consume(receipt: FlashLoanReceipt) {
        let FlashLoanReceipt { pool_id: _, amount: _ } = receipt;
    }

    fun repay(receipt: FlashLoanReceipt) {
        let FlashLoanReceipt { pool_id: _, amount: _ } = receipt;
    }
}
