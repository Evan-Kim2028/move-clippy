/// Fixture for `copyable_fungible_type` (Experimental, full-mode).

module sui::object {
    public struct UID has copy, store {
        id: address,
    }

    public fun new(_ctx: &mut sui::tx_context::TxContext): UID {
        UID { id: @0x0 }
    }
}

module sui::tx_context {
    public struct TxContext has drop {}
}

module copyable_fungible_type_pkg::transfer {
    public fun transfer<T>(obj: T, _recipient: address): T {
        obj
    }

    public fun public_transfer<T>(obj: T, _recipient: address): T {
        obj
    }
}

module copyable_fungible_type_pkg::cases {
    use copyable_fungible_type_pkg::transfer;
    use sui::object;
    use sui::tx_context::TxContext;

    public struct CopyKey has copy, key {
        id: object::UID,
    }

    public struct CopyStore has copy, store {
        value: u64,
    }

    public struct CopyStoreNotTransferred has copy, store {
        value: u64,
    }

    public struct DataOnly has copy, drop {
        value: u64,
    }

    public fun transfer_key(ctx: &mut TxContext, recipient: address): CopyKey {
        let obj = CopyKey { id: object::new(ctx) };
        transfer::transfer(obj, recipient)
    }

    public fun transfer_store(recipient: address): CopyStore {
        let value = CopyStore { value: 10 };
        transfer::public_transfer(value, recipient)
    }

    public fun no_transfer(): (CopyStoreNotTransferred, DataOnly) {
        let value = CopyStoreNotTransferred { value: 1 };
        let data = DataOnly { value: 2 };
        (value, data)
    }
}
