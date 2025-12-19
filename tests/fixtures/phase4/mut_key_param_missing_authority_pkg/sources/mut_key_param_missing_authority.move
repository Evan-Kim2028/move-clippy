/// Fixture for `mut_key_param_missing_authority` (Preview, full-mode).

module sui::object {
    public struct UID has store {
        id: address,
    }

    public fun new(_ctx: &mut sui::tx_context::TxContext): UID {
        UID { id: @0x0 }
    }
}

module sui::tx_context {
    public struct TxContext has drop {}
}

module mut_key_param_missing_authority_pkg::cases {
    use sui::object::UID;
    use sui::tx_context::TxContext;

    public struct State has key {
        id: UID,
        value: u64,
    }

    public struct AdminCap has key, store {
        id: UID,
    }

    public entry fun positive_missing_authority(state: &mut State, _ctx: &mut TxContext) {
        state.value = state.value + 1;
    }

    public entry fun negative_with_cap(state: &mut State, _cap: &AdminCap, _ctx: &mut TxContext) {
        state.value = state.value + 1;
    }

    #[ext(move_clippy(allow(mut_key_param_missing_authority)))]
    public entry fun suppressed(state: &mut State, _ctx: &mut TxContext) {
        state.value = state.value + 1;
    }
}
