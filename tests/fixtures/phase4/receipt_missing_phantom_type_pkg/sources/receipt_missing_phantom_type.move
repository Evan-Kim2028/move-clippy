/// Fixture for `receipt_missing_phantom_type` (Experimental, full-mode).

module sui::coin {
    public struct Coin<phantom T> has store {
        value: u64,
    }

    public fun destroy<T>(coin: Coin<T>): u64 {
        let Coin { value } = coin;
        value
    }
}

module sui::balance {
    public struct Balance<phantom T> has store {
        value: u64,
    }

    public fun destroy<T>(balance: Balance<T>): u64 {
        let Balance { value } = balance;
        value
    }
}

module receipt_missing_phantom_type_pkg::cases {
    use sui::balance::Balance;
    use sui::coin::Coin;

    public struct PaymentReceipt {
        amount: u64,
    }

    public struct PaymentReceiptOk<phantom T> {
        amount: u64,
    }

    public fun purchase<CoinType>(payment: Coin<CoinType>): PaymentReceipt {
        let amount = sui::coin::destroy(payment);
        PaymentReceipt { amount }
    }

    public fun purchase_balance<CoinType>(payment: Balance<CoinType>): PaymentReceipt {
        let amount = sui::balance::destroy(payment);
        PaymentReceipt { amount }
    }

    public fun purchase_ok<CoinType>(payment: Coin<CoinType>): PaymentReceiptOk<CoinType> {
        let amount = sui::coin::destroy(payment);
        PaymentReceiptOk { amount }
    }

    public fun returns_coin<CoinType>(payment: Coin<CoinType>): Coin<CoinType> {
        payment
    }

    public fun returns_tuple<CoinType>(payment: Coin<CoinType>): (Coin<CoinType>, PaymentReceipt) {
        (payment, PaymentReceipt { amount: 1 })
    }
}
