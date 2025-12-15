// Golden test: abilities_order - NEGATIVE (should NOT trigger lint)
// Description: Struct abilities in canonical order (key, copy, drop, store)

module 0x1::test;

// GOOD: single ability
public struct SingleAbility has key {}

// GOOD: two abilities in order
public struct TwoAbilities has key, store {}

// GOOD: three abilities in order
public struct ThreeAbilities has copy, drop, store {}

// GOOD: all four in order
public struct AllAbilities has key, copy, drop, store {}

// GOOD: subset in order
public struct SubsetAbilities has key, drop {}
