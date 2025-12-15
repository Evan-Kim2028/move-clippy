// Golden test: manual_loop_iteration - POSITIVE (should trigger lint)
// Description: Manual while loop with index instead of loop macros

module 0x1::test {
    public fun bad_manual_loop() {
        let v = vector[1, 2, 3];
        let mut i = 0;
        let len = v.length();
        
        // BAD: Manual while loop with index, should use do_ref! or fold!
        while (i < len) {
            let elem = &v[i];
            // Process elem
            let _ = elem;
            i = i + 1;
        };
    }

    public fun bad_manual_loop_mut() {
        let mut v = vector[1, 2, 3];
        let mut i = 0;
        
        // BAD: Manual mutable iteration
        while (i < v.length()) {
            let elem = &mut v[i];
            *elem = *elem + 1;
            i = i + 1;
        };
    }
}
