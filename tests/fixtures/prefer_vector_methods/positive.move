module my_pkg::m;

fun f() {
    let mut v = vector::empty<u64>();
    vector::push_back(&mut v, 1);
    let _n = vector::length(&v);
}
