// Golden test: constant_naming - NEGATIVE (should NOT trigger lint)
// Description: Constants following proper naming conventions

module 0x1::test {
    // GOOD: regular constant in SCREAMING_SNAKE_CASE
    const MAX_VALUE: u64 = 100;
    const MIN_BALANCE: u64 = 50;
    const DEFAULT_SUPPLY: u64 = 1000000;

    // GOOD: error codes with E_ prefix
    const E_INSUFFICIENT_BALANCE: u64 = 1;
    const E_NOT_AUTHORIZED: u64 = 2;
    const E_INVALID_AMOUNT: u64 = 3;

    // GOOD: witness constants in PascalCase
    const WITNESS: bool = true;
}
