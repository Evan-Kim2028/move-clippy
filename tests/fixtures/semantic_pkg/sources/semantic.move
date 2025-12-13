module semantic_pkg::semantic {
    /// Should be suffixed with _cap.
    public struct Admin has key, store {}

    /// Should be <past_tense>_<noun>_event.
    public struct TransferEvent has copy, drop {}

    public struct Holder has key, store {
        value: u64,
    }

    /// Simple getter: should not use get_ prefix.
    public fun get_value(h: &Holder): u64 {
        h.value
    }
}
