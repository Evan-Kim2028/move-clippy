module sui::event;
    // Minimal stub for semantic fixture
    public fun emit<T: copy + drop>(_e: T) {}
}

module test::event_emit_type_sanity_negative;
    use sui::event;

    public struct TransferOccurredEvent has copy, drop {
        amount: u64,
    }

    public fun emit_ok() {
        event::emit<TransferOccurredEvent>(TransferOccurredEvent { amount: 10 });
    }
}
