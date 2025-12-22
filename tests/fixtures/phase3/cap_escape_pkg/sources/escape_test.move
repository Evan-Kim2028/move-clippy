module cap_escape_pkg::escape_test {
    use sui::object::{Self, UID};
    use sui::transfer;
    use sui::tx_context::TxContext;

    /// Admin capability - should not escape to arbitrary addresses
    public struct AdminCap has key, store {
        id: UID,
    }

    /// Treasury capability
    public struct TreasuryCap has key, store {
        id: UID,
    }

    /// Pool object
    public struct Pool has key {
        id: UID,
        admin: address,
    }

    // =========================================================================
    // DANGEROUS: Capability escapes to parameter address (should trigger lint)
    // =========================================================================
    
    /// Dangerous: transfers admin cap to arbitrary address from parameter
    public fun dangerous_transfer_to_param(cap: AdminCap, recipient: address) {
        transfer::public_transfer(cap, recipient);
    }

    /// Dangerous: no authorization check before transfer
    public fun dangerous_unconditional_transfer(cap: AdminCap, ctx: &mut TxContext) {
        let sender = tx_context::sender(ctx);
        transfer::public_transfer(cap, sender);
    }

    // =========================================================================
    // SAFE: Authorized transfers (should NOT trigger lint)
    // =========================================================================

    /// Safe: transfers to constant address
    public fun safe_transfer_to_constant(cap: AdminCap) {
        transfer::public_transfer(cap, @0x123);
    }

    /// Safe: has authorization check before transfer
    public fun safe_transfer_with_auth(
        cap: AdminCap, 
        pool: &Pool,
        recipient: address,
        _auth: &TreasuryCap,  // Authorization capability
    ) {
        // Authorization via capability parameter
        transfer::public_transfer(cap, recipient);
    }

    /// Safe: validates before transfer
    public fun safe_transfer_with_validation(
        cap: AdminCap,
        pool: &Pool,
        recipient: address,
    ) {
        // Authorization check
        assert!(pool.admin == recipient, 0);
        transfer::public_transfer(cap, recipient);
    }

    // =========================================================================
    // DANGEROUS: Sharing capabilities (should trigger lint)
    // =========================================================================

    /// Dangerous: sharing a capability makes it accessible to anyone
    public fun dangerous_share_cap(cap: AdminCap) {
        transfer::public_share_object(cap);
    }

    // =========================================================================
    // Edge cases
    // =========================================================================

    /// Return capability to caller (safe - not an escape)
    public fun return_cap(ctx: &mut TxContext): AdminCap {
        AdminCap { id: object::new(ctx) }
    }
}
