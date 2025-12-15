module test::opt {
    use std::option::{Self, Option};

    public fun test(opt: Option<u64>): u64 {
        opt.destroy_with_default(0)
    }
}
