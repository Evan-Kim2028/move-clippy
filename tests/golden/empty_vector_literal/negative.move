// Golden test: empty_vector_literal - NEGATIVE (should NOT trigger lint)
// Description: Using vector[] literal (correct form)

module 0x1::test;

public fun good_empty_vector(): vector<u64> {
    vector[]
}

public fun good_empty_vector_typed(): vector<u8> {
    vector<u8>[]
}

public fun good_with_elements(): vector<u64> {
    vector[1, 2, 3]
}
