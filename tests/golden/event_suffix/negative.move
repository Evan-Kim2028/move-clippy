module test::events {
    struct TokenMintedEvent has copy, drop {
        amount: u64
    }

    struct UserRegisteredEvent has copy, drop {
        user_id: u64
    }

    struct PoolCreated has copy, drop {
        pool_id: u64
    }

    struct TokenMetadata has key, store {
        id: u64,
        name: vector<u8>
    }

    struct Config has store {
        value: u64
    }
}
