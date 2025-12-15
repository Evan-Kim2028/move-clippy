module my_package::my_module_tests {
    #[test]
    fun addition() {
        assert!(1 + 1 == 2, 0);
    }

    #[test]
    fun subtraction() {
        assert!(5 - 3 == 2, 0);
    }

    #[test]
    fun multiplication() {
        assert!(2 * 3 == 6, 0);
    }
}

module my_package::my_module {
    #[test]
    fun test_feature() {
        assert!(true, 0);
    }
}
