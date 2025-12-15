#[test_only]
module my_package::my_tests {
    #[test]
    fun bad_numeric_abort() {
        let x = 42;
        assert!(x == 42, 0);
        assert!(x > 0, 1);
    }

    #[test]
    fun bad_low_error_codes() {
        assert!(true, 0);
        assert!(1 + 1 == 2, 1);
    }

    #[test]
    fun bad_another_test() {
        assert!(2 + 2 == 4, 123);
    }
}
