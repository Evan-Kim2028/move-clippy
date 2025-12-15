/// Test fixture for share_owned_authority lint.
/// 
/// This lint detects when objects with `key + store` abilities are shared.
/// These objects represent "transferable authority" and sharing them makes
/// them publicly accessible, which is dangerous for capability-like objects.
///
/// The detection is TYPE-GROUNDED (not name-based):
/// - key = object has identity (UID)
/// - store = object can be transferred
/// - key + store together = "transferable authority"

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
    
    public fun sender(_ctx: &TxContext): address {
        @0x0
    }
}

module sui::transfer {
    public native fun transfer<T: key>(obj: T, recipient: address);
    public native fun share_object<T: key>(obj: T);
    public native fun public_share_object<T: key>(obj: T);
    public native fun freeze_object<T: key>(obj: T);
}

// =============================================================================
// POSITIVE CASES - Should trigger share_owned_authority
// =============================================================================

module share_owned_authority_pkg::positive_cases {
    use sui::object::UID;
    use sui::transfer;
    use sui::tx_context::TxContext;

    /// AdminCap has key+store - sharing is dangerous!
    /// SHOULD FIRE: key+store capability being shared
    public struct AdminCap has key, store {
        id: UID,
    }

    /// TreasuryCap has key+store - sharing is dangerous!
    /// SHOULD FIRE: key+store capability being shared
    public struct TreasuryCap has key, store {
        id: UID,
        supply: u64,
    }

    /// Generic authority object - key+store
    /// SHOULD FIRE: key+store authority being shared
    public struct Authority has key, store {
        id: UID,
        permissions: u64,
    }

    /// Sharing AdminCap - DANGEROUS
    public fun share_admin_cap(cap: AdminCap) {
        // LINT: share_owned_authority should fire here
        transfer::share_object(cap);
    }

    /// Sharing TreasuryCap - DANGEROUS  
    public fun share_treasury_cap(cap: TreasuryCap) {
        // LINT: share_owned_authority should fire here
        transfer::share_object(cap);
    }

    /// Using public_share_object variant
    public fun public_share_authority(auth: Authority) {
        // LINT: share_owned_authority should fire here
        transfer::public_share_object(auth);
    }

    /// Sharing in init function - common mistake
    public fun bad_init(ctx: &mut TxContext) {
        let cap = AdminCap { 
            id: sui::object::new(ctx),
        };
        // LINT: share_owned_authority should fire here
        transfer::share_object(cap);
    }
}

// =============================================================================
// NEGATIVE CASES - Should NOT trigger share_owned_authority
// =============================================================================

module share_owned_authority_pkg::negative_cases {
    use sui::object::UID;
    use sui::transfer;
    use sui::tx_context::{Self, TxContext};

    /// key-only object (no store) - safe to share
    /// NOT a "transferable authority" - can't be transferred after creation
    public struct SharedState has key {
        id: UID,
        data: u64,
    }

    /// store-only object (no key) - not an object at all
    /// Just data that can be stored in other objects
    public struct StorableData has store {
        value: u64,
    }

    /// copy+drop object - event-like, not an object
    public struct Event has copy, drop {
        data: u64,
    }

    /// Sharing key-only is fine - it's intentional shared state
    public fun share_state(state: SharedState) {
        // NO LINT: key-only objects are meant to be shared
        transfer::share_object(state);
    }

    /// Transferring key+store is fine - ownership is preserved
    public struct TransferableCap has key, store {
        id: UID,
    }

    public fun transfer_cap(cap: TransferableCap, ctx: &TxContext) {
        // NO LINT: transfer preserves ownership
        transfer::transfer(cap, tx_context::sender(ctx));
    }

    /// Freezing key+store is fine - becomes immutable, not public-writable
    public struct Config has key, store {
        id: UID,
        settings: u64,
    }

    public fun freeze_config(config: Config) {
        // NO LINT: freeze makes it read-only, not writable by anyone
        transfer::freeze_object(config);
    }
}

// =============================================================================
// SUPPRESSION CASES - Intentional sharing with annotation
// =============================================================================

module share_owned_authority_pkg::suppression_cases {
    use sui::object::UID;
    use sui::transfer;

    /// Kiosk is key+store but intentionally shared for marketplace
    public struct Kiosk has key, store {
        id: UID,
        owner: address,
    }

    /// Intentional sharing with suppression annotation
    #[allow(lint(share_owned_authority))]
    public fun create_shared_kiosk(kiosk: Kiosk) {
        // NO LINT: suppressed - developer acknowledges the design choice
        transfer::share_object(kiosk);
    }
}
