module test::opt {
    use std::option::{Self, Option};

    public fun test(opt: Option<u64>): u64 {
        if (opt.is_some()) {
            opt.destroy_some()
        } else {
            0
        }
    }
}
