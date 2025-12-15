// Test fixture for flashloan_without_repay lint
// Tests detection of flashloans not repaid on all paths

module flashloan_pkg::lending {
    // A "hot potato" resource: no `drop` ability.
    public struct HotPotato has store {
        v: u64,
    }

    // Repay consumes a hot potato by-value (so CallGraph marks it as FlashLoan consumer).
    public fun flash_loan_repay(payment: HotPotato): u64 {
        let HotPotato { v } = payment;
        v
    }

    // SHOULD WARN: creator-like function (takes hot potato by value and returns hot potato)
    // that does not consume (repay) on any path.
    public fun bad_flashloan(loan: HotPotato): HotPotato {
        loan
    }

    // SHOULD NOT WARN: consumes the hot potato.
    public fun good_flashloan(loan: HotPotato): u64 {
        flash_loan_repay(loan)
    }
}
