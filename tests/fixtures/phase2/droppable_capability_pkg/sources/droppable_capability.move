/// Fixture package for the `droppable_capability` semantic lint.
///
/// The lint fires on structs with `key + store + drop` and NOT `copy`.

module sui::object {
    /// Test-only UID shim (real Sui UID is not droppable/copyable).
    public struct UID has drop, store {
        v: u64,
    }
}

module droppable_capability_pkg::cases {
    use sui::object::UID;

    // Positive: key + store + drop (no copy)
    public struct Droppable has key, drop, store {
        id: UID,
        v: u64,
    }

    // Negative: key + store (no drop)
    public struct Safe has key, store {
        id: UID,
        v: u64,
    }
}

