/// Fixture for `unbounded_iteration_over_param_vector` (Preview, full-mode).

module sui::tx_context {
    public struct TxContext has drop {}
}

module std::vector {
    public fun length<T>(_v: &vector<T>): u64 {
        0
    }
}

module unbounded_iteration_over_param_vector_pkg::cases {
    use std::vector;
    use sui::tx_context::TxContext;

    public entry fun positive(v: vector<u64>, _ctx: &mut TxContext) {
        let mut i = 0u64;
        while (i < vector::length(&v)) {
            i = i + 1;
        };
    }

    public entry fun negative(v: vector<u64>, _ctx: &mut TxContext) {
        let mut i = 0u64;
        while (i < 10u64) {
            i = i + 1;
        };
    }

    #[ext(move_clippy(allow(unbounded_iteration_over_param_vector)))]
    public entry fun suppressed(v: vector<u64>, _ctx: &mut TxContext) {
        let mut i = 0u64;
        while (i < vector::length(&v)) {
            i = i + 1;
        };
    }
}
