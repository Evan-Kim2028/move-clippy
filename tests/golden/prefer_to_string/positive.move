// Golden test: prefer_to_string - POSITIVE (should trigger lint)
// Description: Importing std::string::utf8 instead of using b"...".to_string()

module 0x1::test {
    // BAD: importing utf8 function
    use std::string::utf8;

    // BAD: another form
    use std::string::{utf8};

    public fun example() {
        let s = utf8(b"hello");
    }
}
