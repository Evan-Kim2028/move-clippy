module test::events {
    struct TokenMinted has copy, drop {
        amount: u64
    }

    struct UserRegistered has copy, drop {
        user_id: u64
    }

    struct PoolCreated has copy, drop {
        pool_id: u64
    }
}
