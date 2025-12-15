// Golden test: merge_test_attributes - NEGATIVE (should NOT trigger lint)
// Description: Merged test attributes

module 0x1::test {
    // GOOD: Merged attributes
    #[test, expected_failure]
    fun good_merged_attributes() {
        abort 42
    }

    // GOOD: Merged with abort code
    #[test, expected_failure(abort_code = 1)]
    fun good_merged_with_code() {
        abort 1
    }

    // GOOD: Just test attribute (no expected_failure)
    #[test]
    fun good_simple_test() {
        assert!(true, 0);
    }

    // GOOD: Not adjacent (has other attributes between)
    #[test]
    #[allow(lint::some_lint)]
    #[expected_failure]
    fun good_not_adjacent() {
        abort 1
    }
}
