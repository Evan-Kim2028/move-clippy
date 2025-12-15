// Golden test: prefer_to_string - NEGATIVE (should NOT trigger lint)
// Description: Using b"...".to_string() (correct form)

module 0x1::test {
    use std::string::String;

    public fun good_to_string(): String {
        // GOOD: using to_string() method without importing utf8
        b"hello".to_string()
    }

    public fun good_multiple(): String {
        let s1 = b"first".to_string();
        let s2 = b"second".to_string();
        s1
    }
}
