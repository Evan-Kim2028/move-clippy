/// Fixture package for the `copyable_capability` semantic lint.
///
/// The lint fires on structs with `key + store + copy`.

module sui::object {
    /// Test-only UID shim (real Sui UID is not droppable/copyable).
    public struct UID has copy, drop, store {
        v: u64,
    }
}

module copyable_capability_pkg::cases {
    use sui::object::UID;

    // Positive: key + store + copy
    public struct Copyable has key, copy, store {
        id: UID,
        v: u64,
    }

    // Also positive: key + store + copy + drop
    public struct CopyableWithDrop has key, copy, drop, store {
        id: UID,
        v: u64,
    }

    // Negative: key + store (no copy)
    public struct Safe has key, store {
        id: UID,
        v: u64,
    }
}
