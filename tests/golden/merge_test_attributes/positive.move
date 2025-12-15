// Golden test: merge_test_attributes - POSITIVE (should trigger lint)
// Description: Separate #[test] and #[expected_failure] attributes

module 0x1::test {
    // BAD: Separate attributes instead of merged
    #[test]
    #[expected_failure]
    fun bad_separate_attributes() {
        abort 42
    }

    // BAD: Another example
    #[test]
    #[expected_failure(abort_code = 1)]
    fun bad_with_abort_code() {
        abort 1
    }
}
