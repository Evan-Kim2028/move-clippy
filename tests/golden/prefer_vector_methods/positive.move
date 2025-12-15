// Golden test: prefer_vector_methods - POSITIVE (should trigger lint)
// Description: Using vector::function(&v) instead of v.function()

module 0x1::test {
    use std::vector;

    public fun bad_push_back() {
        let mut v = vector::empty<u64>();
        // BAD: should use v.push_back(42)
        vector::push_back(&mut v, 42);
    }

    public fun bad_length(): u64 {
        let v = vector[1, 2, 3];
        // BAD: should use v.length()
        vector::length(&v)
    }

    public fun bad_is_empty(): bool {
        let v = vector::empty<u8>();
        // BAD: should use v.is_empty()
        vector::is_empty(&v)
    }

    public fun bad_pop_back() {
        let mut v = vector[1, 2, 3];
        // BAD: should use v.pop_back()
        vector::pop_back(&mut v);
    }
}
