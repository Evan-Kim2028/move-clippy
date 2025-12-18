## tests/support (Shared Integration Test Helpers)

This directory contains small helpers used by integration tests under `tests/`.

### `semantic_spec_harness`

`semantic_spec_harness.rs` provides utilities for creating temporary Move packages on disk for
compiler-backed tests (full mode).

Typical usage in a standalone integration test (`tests/foo_spec.rs`):

```rust
#![cfg(feature = "full")]

mod support;

use support::semantic_spec_harness::create_temp_package;
```

### Module Resolution Note (`mod support;` vs `#[path = ...] mod support;`)

Most integration tests live directly under `tests/` and can use `mod support;` to include
`tests/support/mod.rs`.

If a test file is also included as a module by another test crate (e.g. `tests/spec_tests.rs`
includes `mod droppable_hot_potato;`), then `mod support;` inside that module would resolve
relative to the module file’s directory (e.g. `tests/droppable_hot_potato/support/...`).

In those cases, prefer:

```rust
#[cfg(feature = "full")]
#[path = "support/mod.rs"]
mod support;
```

This makes the inclusion explicit and robust to being compiled both as a standalone integration
test and as a submodule.
