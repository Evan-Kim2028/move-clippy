module my_pkg::m;

#[test]
// comment breaks adjacency; conservative lint should not fire
#[expected_failure]
fun test_fails() {
    abort 0;
}
