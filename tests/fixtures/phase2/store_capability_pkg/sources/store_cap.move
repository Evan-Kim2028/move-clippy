/// Test fixture for store_capability lint.
/// 
/// This lint detects capability-like structs with `store` ability.
/// Capabilities should generally not have `store` as it allows them to be
/// placed in dynamic fields, potentially leaking authorization.
///
/// Tier 2 (Near-Zero FP): Uses naming heuristic (Cap, Admin, Auth, Witness).

module sui::object {
    public struct UID has store {
        id: address,
    }

    public fun new(_ctx: &mut sui::tx_context::TxContext): UID {
        UID { id: @0x0 }
    }

    public fun delete(id: UID) {
        let UID { id: _ } = id;
    }
}

module sui::tx_context {
    public struct TxContext has drop {}
}

module sui::dynamic_field {
    public native fun add<K: copy + drop + store, V: store>(
        object: &mut sui::object::UID,
        key: K,
        value: V
    );
}

module store_capability_pkg::test_cases {
    use sui::object::UID;

    // ==========================================================================
    // POSITIVE CASES - Should trigger store_capability
    // ==========================================================================

    /// AdminCap with store - dangerous!
    /// SHOULD FIRE: "Cap" in name + store ability
    public struct AdminCap has key, store {
        id: UID,
    }

    /// MintCapability with store
    /// SHOULD FIRE: "Capability" in name + store ability
    public struct MintCapability has key, store {
        id: UID,
        max_supply: u64,
    }

    /// AdminAuthority with store
    /// SHOULD FIRE: "Admin" in name + store ability
    public struct AdminAuthority has key, store {
        id: UID,
        level: u8,
    }

    /// AuthToken with store
    /// SHOULD FIRE: "Auth" in name + store ability
    public struct AuthToken has key, store {
        id: UID,
        permissions: u64,
    }

    /// OtwWitness with store (unusual but detectable)
    /// SHOULD FIRE: "Witness" in name + store ability
    public struct OtwWitness has store {
        dummy: bool,
    }

    // ==========================================================================
    // NEGATIVE CASES - Should NOT trigger store_capability
    // ==========================================================================

    /// AdminCap without store - correct pattern
    public struct SafeAdminCap has key {
        id: UID,
    }

    /// Capability without store - correct
    public struct SafeCapability has key {
        id: UID,
    }

    /// Non-capability struct with store - fine
    public struct UserProfile has key, store {
        id: UID,
        name: vector<u8>,
    }

    /// Token with store - not a capability name
    public struct Token has key, store {
        id: UID,
        value: u64,
    }

    /// Pool with store - legitimate storage
    public struct Pool has key, store {
        id: UID,
        reserves: u64,
    }

    /// Capacity (not Capability) with store - fine
    public struct Capacity has key, store {
        id: UID,
        max: u64,
    }

    /// Recap (not Cap suffix) with store - fine
    public struct Recap has key, store {
        id: UID,
        summary: vector<u8>,
    }
}
