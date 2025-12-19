/// Test fixture for missing_witness_drop_v2 lint.
///
/// This lint detects one-time witness (OTW) structs that are missing the `drop` ability.
/// OTW structs must have `drop` so they can be consumed after use in the init function.
///
/// The detection is TYPE-BASED (not name-based):
/// - Uses compiler's module context to verify struct name matches module name in UPPERCASE
/// - Checks struct has no fields (empty body)
/// - Verifies drop ability is present

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

// =============================================================================
// POSITIVE CASES - Should trigger missing_witness_drop_v2
// =============================================================================

/// Module where struct name matches module name (UPPERCASE) but missing drop
module missing_witness_drop_v2_pkg::my_token {
    /// OTW struct without drop - SHOULD FIRE
    /// Struct name MY_TOKEN matches module name my_token in uppercase
    public struct MY_TOKEN {}
}

/// Another module with OTW pattern violation
module missing_witness_drop_v2_pkg::another_coin {
    /// OTW struct without drop - SHOULD FIRE
    public struct ANOTHER_COIN {}
}

// =============================================================================
// NEGATIVE CASES - Should NOT trigger missing_witness_drop_v2
// =============================================================================

/// Module with correct OTW pattern
module missing_witness_drop_v2_pkg::good_token {
    /// Correct OTW with drop ability - NO LINT
    public struct GOOD_TOKEN has drop {}
}

/// Module where struct name doesn't match module name
module missing_witness_drop_v2_pkg::some_module {
    /// Not an OTW - struct name doesn't match module name - NO LINT
    public struct DIFFERENT_NAME {}
    
    /// Regular struct with fields - NO LINT
    public struct SOME_MODULE {
        value: u64,
    }
}

/// Module with struct that has fields (not an OTW)
module missing_witness_drop_v2_pkg::data_struct {
    /// Has fields, so not an OTW pattern - NO LINT
    public struct DATA_STRUCT {
        data: u64,
    }
}

/// Module with lowercase struct (not OTW pattern)
module missing_witness_drop_v2_pkg::lowercase_mod {
    /// Lowercase struct name - not OTW pattern - NO LINT
    public struct lowercase_mod has drop {}
}

// =============================================================================
// SUPPRESSION CASES
// =============================================================================

module missing_witness_drop_v2_pkg::suppressed_token {
    /// Suppressed OTW violation - NO LINT
    #[allow(lint(missing_witness_drop_v2))]
    public struct SUPPRESSED_TOKEN {}
}
