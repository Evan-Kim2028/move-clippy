// Golden test: missing_witness_drop - NEGATIVE (should NOT trigger lint)
// Description: Proper OTW with drop ability

module 0x1::test {
    // GOOD: OTW with drop ability and empty body
    public struct TEST has drop {}

    // GOOD: OTW with drop and copy, empty body
    public struct WITNESS has copy, drop {}

    // GOOD: Regular struct with fields (not OTW pattern) without drop is OK
    public struct RegularStruct {
        value: u64,
    }

    // GOOD: PascalCase (not SCREAMING_SNAKE_CASE) doesn't need drop
    public struct TokenMetadata {
        name: vector<u8>,
    }
}
