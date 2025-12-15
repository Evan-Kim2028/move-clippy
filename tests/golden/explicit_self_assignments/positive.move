// Golden test: explicit_self_assignments - POSITIVE (should trigger lint)
// Description: Using explicit : _ instead of ..

module 0x1::test {
    public struct Point {
        x: u64,
        y: u64,
        z: u64,
    }

    public fun bad_explicit_ignores() {
        let p = Point { x: 1, y: 2, z: 3 };
        // BAD: Should use .. instead of explicit : _
        let Point { x, y: _, z: _ } = p;
        let _ = x;
    }

    public fun bad_all_explicit() {
        let p = Point { x: 1, y: 2, z: 3 };
        // BAD: Multiple explicit ignores
        let Point { x: _, y: _, z } = p;
        let _ = z;
    }
}
