module my_pkg::m;

use std::vector;

#[allow(lint::prefer_vector_methods)]
fun f() {
    let mut v = vector::empty<u8>();
    vector::push_back(&mut v, 1);
    vector::length(&v);
}
