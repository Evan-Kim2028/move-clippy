/// Fixture for `shared_capability_object` (Preview, full-mode).

module sui::object {
    public struct UID has store, drop {
        id: address,
    }

    public fun new(_ctx: &mut sui::tx_context::TxContext): UID {
        UID { id: @0x0 }
    }
}

module sui::tx_context {
    public struct TxContext has drop {}
}

module sui::transfer {
    public native fun share_object<T: key>(obj: T);
    public native fun public_share_object<T: key>(obj: T);
}

module shared_capability_object_pkg::cases {
    use sui::object::UID;
    use sui::transfer;

    /// Capability-like: key+store and not copy/drop (default).
    public struct AdminCap has key, store {
        id: UID,
    }

    /// Not capability-like for this lint: has `drop`.
    public struct SharedState has key {
        id: UID,
        v: u64,
    }

    public fun positive(cap: AdminCap) {
        transfer::share_object(cap);
    }

    public fun negative(state: SharedState) {
        transfer::share_object(state);
    }

    #[ext(move_clippy(allow(shared_capability_object)))]
    public fun suppressed(cap: AdminCap) {
        transfer::public_share_object(cap);
    }
}
