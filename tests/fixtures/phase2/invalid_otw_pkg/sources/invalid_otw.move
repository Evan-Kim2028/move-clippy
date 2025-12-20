/// Fixture package for the `invalid_otw` semantic lint.

module invalid_otw_pkg::token {
    // Valid OTW declaration (used as a negative control).
    public struct TOKEN has drop {}
}

module invalid_otw_pkg::coin {
    // Not an OTW name, so it should be ignored by the lint.
    public struct CoinWitness has drop {
        value: u64,
    }
}

module invalid_otw_pkg::generic {
    // Not an OTW name, so it should be ignored by the lint.
    public struct GenericWitness<T> has drop {}
}
