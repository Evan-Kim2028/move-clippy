module ecosystem::validation {
    use std::vector;

    // ✅ SHOULD TRIGGER: Classic manual iteration pattern
    public fun should_trigger_basic(vec: &vector<u64>): u64 {
        let mut sum = 0;
        let mut i = 0;
        while (i < vec.length()) {
            let elem = vec.borrow(i);
            sum = sum + *elem;
            i = i + 1;
        };
        sum
    }

    // ✅ SHOULD TRIGGER: With early break (still valid for do_ref!)
    public fun should_trigger_with_break(vec: &vector<u64>, limit: u64): u64 {
        let mut sum = 0;
        let mut i = 0;
        while (i < vec.length()) {
            let elem = vec.borrow(i);
            sum = sum + *elem;
            if (sum >= limit) break;
            i = i + 1;
        };
        sum
    }

    // ❌ SHOULD NOT TRIGGER: No borrow pattern (false positive caught!)
    public fun should_not_trigger_no_borrow(vec: &vector<u64>): u64 {
        let mut i = 0;
        while (i < vec.length()) {
            process_index(i);  // Uses index but doesn't borrow
            i = i + 1;
        };
        i
    }

    // ❌ SHOULD NOT TRIGGER: Already using do_ref!
    public fun should_not_trigger_modern(vec: &vector<u64>): u64 {
        let mut sum = 0;
        vec.do_ref!(|elem| {
            sum = sum + *elem;
        });
        sum
    }

    // ❌ SHOULD NOT TRIGGER: Missing increment
    public fun should_not_trigger_no_increment(vec: &vector<u64>) {
        let mut i = 0;
        while (i < vec.length()) {
            let elem = vec.borrow(i);
            process(*elem);
            // Missing increment (creates infinite loop!)
        };
    }

    // ❌ SHOULD NOT TRIGGER: Wrong variable incremented
    public fun should_not_trigger_wrong_var(vec: &vector<u64>) {
        let mut i = 0;
        let mut j = 0;
        while (i < vec.length()) {
            let elem = vec.borrow(i);
            process(*elem);
            j = j + 1;  // Wrong variable!
        };
    }

    fun process_index(i: u64) {}
    fun process(x: u64) {}
}
