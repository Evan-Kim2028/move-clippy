module test::eq {
    fun test() {
        let x = 1;
        assert!(x > 0, 0);
        assert!(x < 10, 1);
    }
}
