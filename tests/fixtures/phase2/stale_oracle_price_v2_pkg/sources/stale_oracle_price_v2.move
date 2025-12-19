/// Test fixture for stale_oracle_price_v2 lint.
///
/// This lint detects usage of unsafe oracle price functions from known oracle providers.
/// Functions like `get_price_unsafe` may return stale prices which can lead to:
/// - Incorrect liquidations
/// - Arbitrage opportunities against the protocol
/// - Loss of user funds
///
/// The detection is TYPE-BASED:
/// - Checks for known oracle modules: pyth, price_info, switchboard, supra
/// - Matches specific unsafe function names: get_price_unsafe, price_unsafe

// =============================================================================
// Mock Oracle Modules
// =============================================================================

module pyth::pyth {
    public struct PriceInfo has drop {
        price: u64,
        timestamp: u64,
    }

    /// Unsafe function - may return stale prices
    public fun get_price_unsafe(_info: &PriceInfo): u64 {
        _info.price
    }

    /// Another unsafe variant
    public fun price_unsafe(_info: &PriceInfo): u64 {
        _info.price
    }

    /// Safe function - validates freshness
    public fun get_price_no_older_than(_info: &PriceInfo, _max_age: u64): u64 {
        _info.price
    }
}

module pyth::price_info {
    public struct PriceInfoObject has drop {
        price: u64,
    }

    /// Unsafe function in price_info module
    public fun get_price_unsafe(_info: &PriceInfoObject): u64 {
        _info.price
    }
}

module switchboard::switchboard {
    public struct Aggregator has drop {
        value: u64,
    }

    /// Unsafe function
    public fun get_price_unsafe(_agg: &Aggregator): u64 {
        _agg.value
    }

    /// Safe function
    public fun get_price_with_staleness_check(_agg: &Aggregator, _max_staleness: u64): u64 {
        _agg.value
    }
}

module supra::supra {
    public struct PriceFeed has drop {
        price: u64,
    }

    /// Unsafe function
    public fun get_price_unsafe(_feed: &PriceFeed): u64 {
        _feed.price
    }
}

// =============================================================================
// POSITIVE CASES - Should trigger stale_oracle_price_v2
// =============================================================================

module stale_oracle_price_v2_pkg::positive_cases {
    use pyth::pyth::{Self, PriceInfo};
    use pyth::price_info::{Self, PriceInfoObject};
    use switchboard::switchboard::{Self, Aggregator};
    use supra::supra::{Self, PriceFeed};

    /// Using pyth::get_price_unsafe - SHOULD FIRE
    public fun use_pyth_unsafe(info: &PriceInfo): u64 {
        pyth::get_price_unsafe(info)
    }

    /// Using pyth::price_unsafe - SHOULD FIRE
    public fun use_pyth_price_unsafe(info: &PriceInfo): u64 {
        pyth::price_unsafe(info)
    }

    /// Using price_info::get_price_unsafe - SHOULD FIRE
    public fun use_price_info_unsafe(info: &PriceInfoObject): u64 {
        price_info::get_price_unsafe(info)
    }

    /// Using switchboard::get_price_unsafe - SHOULD FIRE
    public fun use_switchboard_unsafe(agg: &Aggregator): u64 {
        switchboard::get_price_unsafe(agg)
    }

    /// Using supra::get_price_unsafe - SHOULD FIRE
    public fun use_supra_unsafe(feed: &PriceFeed): u64 {
        supra::get_price_unsafe(feed)
    }

    /// Multiple unsafe calls in one function - SHOULD FIRE MULTIPLE TIMES
    public fun multiple_unsafe_calls(
        pyth_info: &PriceInfo,
        switch_agg: &Aggregator,
    ): u64 {
        let p1 = pyth::get_price_unsafe(pyth_info);
        let p2 = switchboard::get_price_unsafe(switch_agg);
        p1 + p2
    }
}

// =============================================================================
// NEGATIVE CASES - Should NOT trigger stale_oracle_price_v2
// =============================================================================

module stale_oracle_price_v2_pkg::negative_cases {
    use pyth::pyth::{Self, PriceInfo};
    use switchboard::switchboard::{Self, Aggregator};

    /// Using safe pyth function - NO LINT
    public fun use_pyth_safe(info: &PriceInfo): u64 {
        pyth::get_price_no_older_than(info, 60)
    }

    /// Using safe switchboard function - NO LINT
    public fun use_switchboard_safe(agg: &Aggregator): u64 {
        switchboard::get_price_with_staleness_check(agg, 60)
    }

    /// User-defined function with similar name - NO LINT
    /// (not from a known oracle module)
    public fun get_price_unsafe(): u64 {
        42
    }

    /// Calling our own get_price_unsafe - NO LINT
    public fun call_own_unsafe(): u64 {
        get_price_unsafe()
    }
}

// =============================================================================
// SUPPRESSION CASES
// =============================================================================

module stale_oracle_price_v2_pkg::suppression_cases {
    use pyth::pyth::{Self, PriceInfo};

    /// Suppressed unsafe call - NO LINT
    #[allow(lint(stale_oracle_price_v2))]
    public fun suppressed_unsafe(info: &PriceInfo): u64 {
        pyth::get_price_unsafe(info)
    }
}
