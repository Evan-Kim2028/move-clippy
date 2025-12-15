// Golden test: droppable_hot_potato - NEGATIVE (should NOT trigger lint)
// Description: Proper hot potato without drop ability

module 0x1::test {
    // GOOD: Hot potato without drop ability
    public struct GoodHotPotato {
        value: u64,
    }

    // GOOD: Hot potato with only copy (no drop)
    public struct FlashLoan has copy {
        amount: u64,
    }

    // GOOD: Regular struct with drop (no hot potato keywords)
    public struct RegularStruct has drop, store {
        data: u64,
    }

    // GOOD: "potato" in name but has both copy AND drop (data transfer object, not enforced hot potato)
    public struct PotatoData has copy, drop, store {
        value: u64,
    }

    // GOOD: "receipt" in name but has both copy AND drop (tracking object, not enforced)
    public struct PurchaseReceipt has copy, drop {
        amount: u64,
    }
}
