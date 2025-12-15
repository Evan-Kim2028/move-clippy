#[test_only]
module my_package::my_tests {
    const E_TEST_FAILED: u64 = 999999;

    #[test]
    fun good_named_constant() {
        let x = 42;
        assert!(x == 42, E_TEST_FAILED);
    }

    #[test]
    fun good_high_number() {
        assert!(true, 999999);
    }

    #[test]
    fun no_assertions() {
        let _ = 42;
    }
}
