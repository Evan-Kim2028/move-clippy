/// Fixture for `droppable_flash_loan_receipt` (Experimental, full-mode).

module sui::object {
    public struct UID has store {
        id: address,
    }

    public fun new(_ctx: &mut sui::tx_context::TxContext): UID {
        UID { id: @0x0 }
    }
}

module sui::tx_context {
    public struct TxContext has drop {}
}

module sui::coin {
    public struct Coin<phantom T> has key, store {
        id: sui::object::UID,
        value: u64,
    }

    public fun new<T>(value: u64, ctx: &mut sui::tx_context::TxContext): Coin<T> {
        Coin {
            id: sui::object::new(ctx),
            value,
        }
    }
}

module sui::balance {
    public struct Balance<phantom T> has store {
        value: u64,
    }

    public fun new<T>(value: u64): Balance<T> {
        Balance { value }
    }
}

module droppable_flash_loan_receipt_pkg::cases {
    use sui::balance::Balance;
    use sui::coin::Coin;

    public struct SUI has drop {}

    /// Droppable receipt - should trigger.
    public struct FlashReceipt has drop {
        amount: u64,
    }

    /// Safe receipt - no abilities.
    public struct FlashReceiptSafe {
        amount: u64,
    }

    public fun borrow_bad(ctx: &mut sui::tx_context::TxContext): (Coin<SUI>, FlashReceipt) {
        let coin = sui::coin::new<SUI>(1, ctx);
        let receipt = FlashReceipt { amount: 1 };
        (coin, receipt)
    }

    public fun borrow_bad_balance(_ctx: &mut sui::tx_context::TxContext): (Balance<SUI>, FlashReceipt) {
        let bal = sui::balance::new<SUI>(10);
        let receipt = FlashReceipt { amount: 10 };
        (bal, receipt)
    }

    public fun borrow_ok(ctx: &mut sui::tx_context::TxContext): (Coin<SUI>, FlashReceiptSafe) {
        let coin = sui::coin::new<SUI>(2, ctx);
        let receipt = FlashReceiptSafe { amount: 2 };
        (coin, receipt)
    }

    public fun no_coin(_ctx: &mut sui::tx_context::TxContext): (FlashReceipt, u64) {
        let receipt = FlashReceipt { amount: 3 };
        (receipt, 3)
    }
}
