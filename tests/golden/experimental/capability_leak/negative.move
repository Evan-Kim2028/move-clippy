// Golden test: capability_leak - NEGATIVE (should NOT trigger)
// Description: Proper capability handling or non-capability transfers

module 0x1::test {
    use sui::transfer;
    use sui::tx_context::TxContext;

    public struct AdminCap has key, store {
        id: UID,
    }

    public struct NFT has key, store {
        id: UID,
        name: vector<u8>,
    }

    // GOOD: Transferring non-capability object
    public fun transfer_nft(nft: NFT, recipient: address) {
        transfer::public_transfer(nft, recipient);
    }

    // GOOD: Transfer to sender (keeping capability with authorized user)
    public fun transfer_to_sender(cap: AdminCap, ctx: &TxContext) {
        transfer::public_transfer(cap, ctx.sender());
    }

    // GOOD: Sharing non-capability object
    public fun share_nft(nft: NFT) {
        transfer::public_share_object(nft);
    }
}
