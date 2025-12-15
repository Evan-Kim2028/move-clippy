// Golden test: shared_capability - POSITIVE (should trigger lint)
// Description: Sharing capability objects (AdminCap, MintCap, etc.)

module 0x1::test;

use sui::transfer;

public struct AdminCap has key, store { id: UID }
public struct MintCap has key, store { id: UID }
public struct TreasuryCap has key, store { id: UID }

// BAD: sharing AdminCap
public fun bad_share_admin_cap(cap: AdminCap) {
    transfer::public_share_object(cap);
}

// BAD: sharing MintCap
public fun bad_share_mint_cap(cap: MintCap) {
    transfer::share_object(cap);
}

// BAD: sharing TreasuryCap
public fun bad_share_treasury(cap: TreasuryCap) {
    transfer::public_share_object(cap);
}
