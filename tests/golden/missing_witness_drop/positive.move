// Golden test: missing_witness_drop - POSITIVE (should trigger lint)
// Description: One-time witness struct missing drop ability

module 0x1::test {
    // BAD: OTW pattern (SCREAMING_SNAKE_CASE name, empty body) without drop
    public struct TEST has copy {}

    // BAD: Another OTW without drop
    public struct MYTOKEN {}
}