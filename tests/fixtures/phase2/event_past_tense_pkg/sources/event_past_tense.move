/// Fixture package for the `event_past_tense` semantic lint.

module sui::event {
    public fun emit<T: drop>(_event: T) {}
}

module event_past_tense_pkg::events {
    use sui::event;

    public struct CreateItem has copy, drop {
        id: u64,
    }

    public struct ItemCreated has copy, drop {
        id: u64,
    }

    public struct MintToken has copy, drop {
        id: u64,
    }

    public fun emit_all() {
        event::emit<CreateItem>(CreateItem { id: 1 });
        event::emit<ItemCreated>(ItemCreated { id: 2 });
        event::emit<MintToken>(MintToken { id: 3 });
    }
}
