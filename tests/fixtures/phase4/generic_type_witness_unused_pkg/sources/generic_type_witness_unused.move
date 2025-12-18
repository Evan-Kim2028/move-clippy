/// Fixture for `generic_type_witness_unused` (Experimental, full-mode).

module sui::tx_context {
    public struct TxContext has drop {}
}

module std::type_name {
    public struct TypeName has copy, drop, store {
        v: u64,
    }

    public fun get<T>(): TypeName {
        TypeName { v: 0 }
    }
}

module generic_type_witness_unused_pkg::cases {
    use std::type_name::TypeName;
    use sui::tx_context::TxContext;

    public fun positive<T>(_witness: TypeName, _ctx: &mut TxContext) {
        // witness unused
    }

    public fun negative<T>(witness: TypeName, _ctx: &mut TxContext) {
        let _ = witness;
    }

    #[ext(move_clippy(allow(generic_type_witness_unused)))]
    public fun suppressed<T>(_witness: TypeName, _ctx: &mut TxContext) {
        // suppressed
    }
}
