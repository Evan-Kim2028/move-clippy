module my_pkg::m;

#[test]
#[expected_failure]
fun test_fails() {
    abort 0;
}
