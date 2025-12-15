module scoping_pkg::root {
    public fun do_work(): u64 {
        dep_pkg::dep::do_dep_work();
        1
    }
}