module test::events {
    struct TokenMinting has copy, drop {
        amount: u64
    }

    struct UserRegistration has copy, drop {
        user_id: u64
    }

    struct PoolCreation has copy, drop {
        pool_id: u64
    }
}
