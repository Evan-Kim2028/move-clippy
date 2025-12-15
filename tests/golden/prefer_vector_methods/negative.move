// Golden test: prefer_vector_methods - NEGATIVE (should NOT trigger lint)
// Description: Using v.function() method syntax

module 0x1::test {
    public fun good_push_back() {
        let mut v = vector::empty<u64>();
        // GOOD: using method syntax
        v.push_back(42);
        v.push_back(100);
    }

    public fun good_length(): u64 {
        let v = vector[1, 2, 3];
        // GOOD: using method syntax
        v.length()
    }

    public fun good_is_empty(): bool {
        let v = vector::empty<u8>();
        // GOOD: using method syntax
        v.is_empty()
    }

    public fun good_pop_back(): u64 {
        let mut v = vector[1, 2, 3];
        // GOOD: using method syntax
        v.pop_back()
    }

    // GOOD: Complex expressions don't trigger
    public fun complex_ok() {
        let len = get_vector().length();
    }
}
