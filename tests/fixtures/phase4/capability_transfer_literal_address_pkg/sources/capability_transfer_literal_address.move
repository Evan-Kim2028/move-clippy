/// Fixture for `capability_transfer_literal_address` (Preview, full-mode).

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

    public fun sender(_ctx: &TxContext): address {
        @0x0
    }
}

module sui::transfer {
    public native fun transfer<T: key>(obj: T, recipient: address);
    public native fun public_transfer<T: key>(obj: T, recipient: address);
}

module capability_transfer_literal_address_pkg::cases {
    use sui::object::UID;
    use sui::transfer;
    use sui::tx_context::{Self, TxContext};

    public struct AdminCap has key, store {
        id: UID,
    }

    /// Not capability-like for this lint: has `drop`.
    public struct Config has key {
        id: UID,
        x: u64,
    }

    public fun positive_literal(cap: AdminCap) {
        transfer::transfer(cap, @0x42);
    }

    public fun negative_sender(cap: AdminCap, ctx: &TxContext) {
        transfer::transfer(cap, tx_context::sender(ctx));
    }

    public fun negative_param(cap: AdminCap, recipient: address) {
        transfer::public_transfer(cap, recipient);
    }

    public fun negative_noncap(cfg: Config) {
        transfer::transfer(cfg, @0x42);
    }

    #[ext(move_clippy(allow(capability_transfer_literal_address)))]
    public fun suppressed_literal(cap: AdminCap) {
        transfer::public_transfer(cap, @0x99);
    }
}
