// Golden test: modern_module_syntax - NEGATIVE (should NOT trigger lint)
// Description: Using modern Move 2024 label syntax

module 0x1::modern_label_form;

use sui::object;

fun example() {
    let x = 42;
}

fun another_example() {
    let y = 100;
}
