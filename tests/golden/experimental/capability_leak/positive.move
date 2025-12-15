// Golden test: capability_leak - POSITIVE (should trigger with --experimental)
// Description: Transferring capability to untrusted recipient

module 0x1::test {
    use sui::transfer;

    public struct AdminCap has key, store {
        id: UID,
    }

    public struct MintCap has key, store {
        id: UID,
    }

    // BAD: Transferring capability without recipient validation
    public fun bad_transfer_admin(cap: AdminCap, recipient: address) {
        transfer::public_transfer(cap, recipient);
    }

    // BAD: Transfer capability to arbitrary address
    public fun transfer_mint_cap(cap: MintCap, to: address) {
        transfer::transfer(cap, to);
    }
}
