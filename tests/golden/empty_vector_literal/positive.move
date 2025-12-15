// Golden test: empty_vector_literal - POSITIVE (should trigger lint)
// Description: Using vector::empty() instead of vector[] literal

module 0x1::test;

public fun bad_empty_vector(): vector<u64> {
    vector::empty<u64>()
}

public fun bad_empty_vector_no_type(): vector<u8> {
    vector::empty()
}

public fun bad_in_assignment() {
    let v = vector::empty<u64>();
    let _ = v;
}
