/// Fixture package for the `capability_antipatterns` semantic lint.

module sui::tx_context {
    public struct TxContext has drop {}
}

module sui::object {
    public struct UID has store, drop {
        id: u64,
    }

    public fun new(_ctx: &mut sui::tx_context::TxContext): UID {
        UID { id: 0 }
    }
}

module capability_antipatterns_pkg::caps {
    public struct CopyCap has copy, drop {}

    public struct StoreCap has store {}

    public struct MintCap has key, store {
        id: sui::object::UID,
    }

    public struct PurchaseCap has key, store {
        id: sui::object::UID,
    }

    public struct MetadataCap has key, store {
        id: sui::object::UID,
    }

    public fun mint_cap(ctx: &mut sui::tx_context::TxContext): MintCap {
        MintCap { id: sui::object::new(ctx) }
    }

    public fun list_with_purchase_cap(ctx: &mut sui::tx_context::TxContext): PurchaseCap {
        PurchaseCap { id: sui::object::new(ctx) }
    }

    public fun claim_metadata_cap(ctx: &mut sui::tx_context::TxContext): MetadataCap {
        MetadataCap { id: sui::object::new(ctx) }
    }

    fun init(ctx: &mut sui::tx_context::TxContext) {
        let cap = MintCap { id: sui::object::new(ctx) };
        let MintCap { id } = cap;
        let _ = id;
    }
}
