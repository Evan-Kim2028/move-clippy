// Golden test: constant_naming - POSITIVE (should trigger lint)
// Description: Constants not following SCREAMING_SNAKE_CASE convention

module 0x1::test {
    // BAD: regular constant not in SCREAMING_SNAKE_CASE
    const max_value: u64 = 100;

    // BAD: error code with wrong prefix
    const E_bad_error: u64 = 1;

    // BAD: mixed case
    const MaxSupply: u64 = 1000;

    // BAD: camelCase
    const errorCode: u64 = 2;
}
