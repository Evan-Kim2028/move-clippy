/// Fixture package for the `non_transferable_fungible_object` semantic lint.
///
/// The lint fires on structs with `key` but NOT `store`, and with `copy` and/or `drop`.

module sui::object {
    /// Test-only UID shim (real Sui UID is not droppable/copyable).
    public struct UID has copy, drop, store {
        v: u64,
    }
}

module non_transferable_fungible_object_pkg::cases {
    use sui::object::UID;

    // Positive: key without store but has copy/drop.
    public struct WeirdCopy has key, copy {
        id: UID,
        v: u64,
    }

    public struct WeirdDrop has key, drop {
        id: UID,
        v: u64,
    }

    public struct WeirdCopyDrop has key, copy, drop {
        id: UID,
        v: u64,
    }

    // Negative: key without store, but also without copy/drop (legitimate soulbound object).
    public struct Soulbound has key {
        id: UID,
        v: u64,
    }
}

