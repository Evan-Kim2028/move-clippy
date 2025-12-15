// Golden test: abilities_order - POSITIVE (should trigger lint)
// Description: Struct abilities not in canonical order (key, copy, drop, store)

module 0x1::test;

// BAD: store before drop
public struct BadOrder1 has store, drop {}

// BAD: copy before key
public struct BadOrder2 has copy, key {}

// BAD: completely reversed
public struct BadOrder3 has store, drop, copy, key {}
