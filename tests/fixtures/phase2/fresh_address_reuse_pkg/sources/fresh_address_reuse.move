module sui::tx_context {
    public struct TxContext has store, drop {
        dummy: bool,
    }

    public fun new(): TxContext {
        TxContext { dummy: true }
    }
}

module sui::object {
    use sui::tx_context::TxContext;

    public struct UID has store, drop {
        id: address,
    }

    public fun fresh_object_address(_ctx: &mut TxContext): address {
        @0x1
    }

    public fun new_uid_from_address(a: address): UID {
        UID { id: a }
    }
}

module fresh_address_reuse_pkg::m {
    use sui::object;
    use sui::tx_context::TxContext;

    public fun bad_reuse(ctx: &mut TxContext) {
        let a = object::fresh_object_address(ctx);
        let _uid1 = object::new_uid_from_address(a);
        let _uid2 = object::new_uid_from_address(a);
    }

    public fun good_no_reuse(ctx: &mut TxContext) {
        let a1 = object::fresh_object_address(ctx);
        let _uid1 = object::new_uid_from_address(a1);

        let a2 = object::fresh_object_address(ctx);
        let _uid2 = object::new_uid_from_address(a2);
    }
}
