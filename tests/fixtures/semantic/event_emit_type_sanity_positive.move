module sui::event;
    // Minimal stub for semantic fixture
    public fun emit<T: copy + drop>(_e: T) {}
}

module test::event_emit_type_sanity_positive;
    use sui::event;

    // Has key, not an event
    public struct NotAnEvent has key {
        id: address,
    }

    // Missing copy/drop
    public struct NotCopyDrop {
        v: u64,
    }

    public struct GoodEvent has copy, drop {
        v: u64,
    }

    public fun bad_emit_key() {
        event::emit<NotAnEvent>(NotAnEvent { id: @0x0 });
    }

    public fun bad_emit_missing_copy_drop() {
        event::emit<NotCopyDrop>(NotCopyDrop { v: 1 });
    }

    public fun ok_emit() {
        event::emit<GoodEvent>(GoodEvent { v: 1 });
    }
}
