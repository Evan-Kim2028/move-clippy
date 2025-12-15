// Golden test: droppable_hot_potato - POSITIVE (should trigger lint)
// Description: Hot potato pattern struct has drop ability

module 0x1::test {
    // BAD: Hot potato should not have drop ability
    public struct BadHotPotato has drop {
        value: u64,
    }

    // BAD: Another hot potato with drop
    public struct AnotherHotPotato has copy, drop {
        data: vector<u8>,
    }
}
