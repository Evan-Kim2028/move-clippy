// Golden test: modern_module_syntax - POSITIVE (should trigger lint)
// Description: Using legacy block syntax instead of modern label syntax

module 0x1::legacy_block_form {
    public fun example() {}
}

module 0x2::another_legacy {
    use sui::object;
    
    public fun test() {}
}
