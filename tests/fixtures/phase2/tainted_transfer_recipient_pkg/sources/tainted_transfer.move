// Test fixture for tainted_transfer_recipient lint
// Tests taint tracking for entry function address parameters flowing to transfer sinks

// Mock Sui framework modules
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
    public native fun public_transfer<T: key + store>(obj: T, recipient: address);
    public native fun share_object<T: key>(obj: T);
    public native fun public_share_object<T: key + store>(obj: T);
}

module sui::coin {
    use sui::object::UID;
    use sui::tx_context::TxContext;
    
    public struct Coin<phantom T> has key, store {
        id: UID,
        value: u64,
    }
    
    public fun split<T>(coin: &mut Coin<T>, amount: u64, _ctx: &mut TxContext): Coin<T> {
        coin.value = coin.value - amount;
        Coin { id: sui::object::new(_ctx), value: amount }
    }
    
    public fun value<T>(coin: &Coin<T>): u64 {
        coin.value
    }
}

module sui::dynamic_field {
    use sui::object::UID;
    
    public native fun add<Name: copy + drop + store, Value: store>(obj: &mut UID, name: Name, value: Value);
    public native fun borrow<Name: copy + drop + store, Value: store>(obj: &UID, name: Name): &Value;
    public native fun borrow_mut<Name: copy + drop + store, Value: store>(obj: &mut UID, name: Name): &mut Value;
    public native fun remove<Name: copy + drop + store, Value: store>(obj: &mut UID, name: Name): Value;
}

// Test module
module tainted_transfer_recipient_pkg::tainted_transfer {
    use sui::object::UID;
    use sui::tx_context::TxContext;
    use sui::transfer;

    const E_UNAUTHORIZED: u64 = 1;

    public struct Coin has key, store {
        id: UID,
        value: u64,
    }

    public struct Vault has key {
        id: UID,
        owner: address,
    }

    // =========================================================================
    // SHOULD WARN: Tainted address flows directly to transfer
    // =========================================================================

    // SHOULD WARN: Direct tainted param to transfer
    public entry fun bad_direct_transfer(coin: Coin, recipient: address) {
        transfer::public_transfer(coin, recipient);
    }

    // SHOULD WARN: Taint flows through variable assignment
    public entry fun bad_aliased_transfer(coin: Coin, recipient: address) {
        let dest = recipient;
        transfer::public_transfer(coin, dest);
    }

    // SHOULD WARN: Taint flows through multiple assignments
    public entry fun bad_multi_hop_transfer(coin: Coin, recipient: address) {
        let a = recipient;
        let b = a;
        let c = b;
        transfer::public_transfer(coin, c);
    }

    // SHOULD WARN: Using transfer::transfer instead of public_transfer
    public entry fun bad_private_transfer(coin: Coin, recipient: address) {
        transfer::transfer(coin, recipient);
    }

    // =========================================================================
    // SHOULD NOT WARN: Validated or intentionally unvalidated
    // =========================================================================

    // SHOULD NOT WARN: Validated with assert! against sender
    public entry fun good_sender_validated(
        coin: Coin,
        recipient: address,
        ctx: &TxContext,
    ) {
        assert!(recipient == sui::tx_context::sender(ctx), E_UNAUTHORIZED);
        transfer::public_transfer(coin, recipient);
    }

    // SHOULD NOT WARN: Validated with assert! against stored owner
    public entry fun good_owner_validated(
        vault: &Vault,
        coin: Coin,
        recipient: address,
    ) {
        assert!(recipient == vault.owner, E_UNAUTHORIZED);
        transfer::public_transfer(coin, recipient);
    }

    // SHOULD NOT WARN: Validated with if-abort pattern
    public entry fun good_if_abort_validated(
        coin: Coin,
        recipient: address,
        ctx: &TxContext,
    ) {
        if (recipient != sui::tx_context::sender(ctx)) {
            abort E_UNAUTHORIZED
        };
        transfer::public_transfer(coin, recipient);
    }

    // SHOULD NOT WARN: Underscore prefix signals intentional
    public entry fun good_intentional_airdrop(coin: Coin, _recipient: address) {
        transfer::public_transfer(coin, _recipient);
    }

    // SHOULD NOT WARN: Hardcoded address (not from parameter)
    public entry fun good_hardcoded_recipient(coin: Coin) {
        let treasury = @0x1234;
        transfer::public_transfer(coin, treasury);
    }

    // SHOULD NOT WARN: Transfer to sender (from context, not param)
    public entry fun good_self_transfer(coin: Coin, ctx: &TxContext) {
        transfer::public_transfer(coin, sui::tx_context::sender(ctx));
    }

    // SHOULD NOT WARN: Non-entry function (not exposed to users)
    public fun internal_transfer(coin: Coin, recipient: address) {
        // This is called by other module code, not directly by users
        transfer::public_transfer(coin, recipient);
    }

    // =========================================================================
    // Edge cases
    // =========================================================================

    // SHOULD NOT WARN: Multiple params but only one is address type
    public entry fun good_amount_param(coin: Coin, _amount: u64, ctx: &TxContext) {
        // amount is not an address type, so not tainted for transfer recipient
        transfer::public_transfer(coin, sui::tx_context::sender(ctx));
    }

    // =========================================================================
    // COIN SPLIT AMOUNT SINK TESTS
    // =========================================================================

    // SHOULD WARN: Tainted amount flows to coin::split
    public entry fun bad_split_tainted_amount(
        coin: &mut sui::coin::Coin<Coin>,
        amount: u64,
        ctx: &mut TxContext,
    ) {
        let split_coin = sui::coin::split(coin, amount, ctx);
        transfer::public_transfer(split_coin, sui::tx_context::sender(ctx));
    }

    // SHOULD NOT WARN: Validated amount flows to coin::split
    public entry fun good_split_validated_amount(
        coin: &mut sui::coin::Coin<Coin>,
        amount: u64,
        max_allowed: u64,
        ctx: &mut TxContext,
    ) {
        assert!(amount <= max_allowed, E_UNAUTHORIZED);
        let split_coin = sui::coin::split(coin, amount, ctx);
        transfer::public_transfer(split_coin, sui::tx_context::sender(ctx));
    }

    // SHOULD NOT WARN: Underscore-prefixed amount is intentional
    public entry fun good_split_intentional_amount(
        coin: &mut sui::coin::Coin<Coin>,
        _amount: u64,
        ctx: &mut TxContext,
    ) {
        let split_coin = sui::coin::split(coin, _amount, ctx);
        transfer::public_transfer(split_coin, sui::tx_context::sender(ctx));
    }

    // =========================================================================
    // DYNAMIC FIELD KEY SINK TESTS
    // =========================================================================

    // SHOULD WARN: Tainted key flows to dynamic_field::add
    public entry fun bad_dynamic_field_add(
        vault: &mut Vault,
        key: u64,
        value: u64,
    ) {
        sui::dynamic_field::add(&mut vault.id, key, value);
    }

    // SHOULD NOT WARN: Validated key
    public entry fun good_dynamic_field_validated(
        vault: &mut Vault,
        key: u64,
        value: u64,
    ) {
        // Whitelist validation
        assert!(key == 1 || key == 2 || key == 3, E_UNAUTHORIZED);
        sui::dynamic_field::add(&mut vault.id, key, value);
    }

    // SHOULD NOT WARN: Underscore-prefixed key is intentional
    public entry fun good_dynamic_field_intentional(
        vault: &mut Vault,
        _key: u64,
        value: u64,
    ) {
        sui::dynamic_field::add(&mut vault.id, _key, value);
    }
}
