module my_package::my_module_tests {
    #[test]
    fun test_addition() {
        assert!(1 + 1 == 2, 0);
    }

    #[test]
    fun test_subtraction() {
        assert!(5 - 3 == 2, 0);
    }

    #[test]
    fun test_multiplication() {
        assert!(2 * 3 == 6, 0);
    }
}
