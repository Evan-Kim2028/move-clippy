// Golden test: shared_capability - NEGATIVE (should NOT trigger lint)
// Description: Proper capability handling (transfer, not share)

module 0x1::test;

use sui::transfer;

public struct AdminCap has key, store { id: UID }
public struct Pool has key, store { id: UID }
public struct Registry has key, store { id: UID }

// GOOD: transferring AdminCap to owner
public fun good_transfer_admin_cap(cap: AdminCap, recipient: address) {
    transfer::public_transfer(cap, recipient);
}

// GOOD: sharing non-capability objects
public fun good_share_pool(pool: Pool) {
    transfer::public_share_object(pool);
}

// GOOD: sharing registry
public fun good_share_registry(registry: Registry) {
    transfer::share_object(registry);
}
