module sui::object {
    public struct UID has drop, store {}
}

module semantic_pkg::semantic {
    use sui::object::UID;

    /// Should be suffixed with _cap.
    public struct Admin has key, store {
        id: UID,
    }

    /// Should be <past_tense>_<noun>_event.
    public struct TransferEvent has copy, drop {}

    public struct Holder has key, store {
        id: UID,
        value: u64,
    }

    /// Simple getter: should not use get_ prefix.
    public fun get_value(h: &Holder): u64 {
        h.value
    }
}
