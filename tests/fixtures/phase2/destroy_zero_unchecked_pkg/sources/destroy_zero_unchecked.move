module sui::balance {
    public struct Balance has store, drop {
        value: u64,
    }

    public fun new(value: u64): Balance {
        Balance { value }
    }

    public fun value(b: &Balance): u64 {
        b.value
    }

    public fun destroy_zero(b: Balance) {
        let Balance { value: _ } = b;
    }
}

module destroy_zero_unchecked_pkg::m {
    use sui::balance;

    public fun bad_destroy_zero(b: balance::Balance) {
        balance::destroy_zero(b);
    }

    public fun good_destroy_zero(b: balance::Balance) {
        if (balance::value(&b) != 0) {
            abort 0;
        };
        balance::destroy_zero(b);
    }

    public fun make_nonzero(): balance::Balance {
        balance::new(1)
    }

    public fun make_zero(): balance::Balance {
        balance::new(0)
    }
}
