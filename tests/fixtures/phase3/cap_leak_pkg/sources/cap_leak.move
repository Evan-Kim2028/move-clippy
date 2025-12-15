// Test fixture for transitive_capability_leak lint
// Tests detection of capabilities flowing across module boundaries

// Minimal stub so this fixture compiles without pulling in the full Sui framework.
module sui::object {
    public struct UID has store, drop {}

    public fun delete(_id: UID) {}
}

module cap_leak_pkg::types {
    use sui::object;
    use sui::object::UID;

    /// Key+store auth token / capability-like resource.
    public struct AdminCap has key, store {
        id: UID,
    }

    public fun destroy(cap: AdminCap) {
        let AdminCap { id } = cap;
        object::delete(id);
    }
}

module cap_leak_pkg::public_api {
    use cap_leak_pkg::types::AdminCap;
    use cap_leak_pkg::types;

    /// Public API that takes a key+store value by-value.
    /// Cross-module callers passing this value should be flagged by `transitive_capability_leak`.
    public fun accept_cap(cap: AdminCap) {
        types::destroy(cap);
    }
}

module cap_leak_pkg::caller {
    use cap_leak_pkg::public_api;
    use cap_leak_pkg::types::AdminCap;

    /// SHOULD WARN: capability value flows into a public API in another module.
    public fun leak_cap(cap: AdminCap) {
        public_api::accept_cap(cap)
    }
}
