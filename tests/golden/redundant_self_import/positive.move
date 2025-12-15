// Golden test: redundant_self_import - POSITIVE (should trigger lint)
// Description: Using use pkg::mod::{Self} instead of use pkg::mod

module 0x1::test {
    // BAD: Redundant {Self} import
    use sui::object::{Self};

    // BAD: Another redundant Self
    use std::vector::{Self};

    public fun example() {
        let id = object::new();
    }
}