// Golden test: explicit_self_assignments - NEGATIVE (should NOT trigger lint)
// Description: Using .. to ignore multiple fields

module 0x1::test {
    public struct Point {
        x: u64,
        y: u64,
        z: u64,
    }

    public fun good_use_dot_dot() {
        let p = Point { x: 1, y: 2, z: 3 };
        // GOOD: Using .. to ignore remaining fields
        let Point { x, .. } = p;
        let _ = x;
    }

    public fun good_extract_all() {
        let p = Point { x: 1, y: 2, z: 3 };
        // GOOD: Extracting all fields explicitly
        let Point { x, y, z } = p;
        let _ = x;
        let _ = y;
        let _ = z;
    }
}
