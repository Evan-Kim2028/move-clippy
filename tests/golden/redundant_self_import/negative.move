// Golden test: redundant_self_import - NEGATIVE (should NOT trigger lint)
// Description: Proper import syntax

module 0x1::test {
    // GOOD: Import module directly
    use sui::object;

    // GOOD: Import specific items
    use std::vector::{empty, push_back};

    // GOOD: Self with other items (not redundant)
    use sui::tx_context::{Self, TxContext};

    public fun example() {
        let v = empty<u64>();
        let ctx: TxContext;
    }
}
