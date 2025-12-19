/// Test fixture for public_random_access_v2 lint.
///
/// This lint detects public (non-entry) functions that expose `sui::random::Random` objects.
/// Random objects should only be accessible in entry functions to prevent
/// front-running attacks where validators can see random values before including transactions.
///
/// The detection is TYPE-BASED (not name-based):
/// - Checks for exact type `sui::random::Random` (0x2::random::Random)
/// - Handles both direct Random params and &Random references
/// - Ignores entry functions (which are allowed to take Random)

module sui::random {
    public struct Random has key {
        id: sui::object::UID,
    }
    
    public struct RandomGenerator has drop {
        seed: vector<u8>,
    }
    
    public fun new_generator(_r: &Random, _ctx: &mut sui::tx_context::TxContext): RandomGenerator {
        RandomGenerator { seed: vector[] }
    }
}

module sui::object {
    public struct UID has store {
        id: address,
    }
}

module sui::tx_context {
    public struct TxContext has drop {}
}

// =============================================================================
// POSITIVE CASES - Should trigger public_random_access_v2
// =============================================================================

module public_random_access_v2_pkg::positive_cases {
    use sui::random::Random;
    
    /// Public function taking &Random - DANGEROUS
    /// SHOULD FIRE: public non-entry function exposes Random
    public fun get_random_number(_r: &Random): u64 {
        // This enables front-running!
        42
    }
    
    /// Public function with &mut Random - DANGEROUS
    /// SHOULD FIRE: public non-entry function exposes Random
    public fun mutate_with_random(_r: &mut Random): u64 {
        42
    }
}

// =============================================================================
// NEGATIVE CASES - Should NOT trigger public_random_access_v2
// =============================================================================

module public_random_access_v2_pkg::negative_cases {
    use sui::random::{Random, RandomGenerator};
    use sui::tx_context::TxContext;
    
    /// Entry function with Random - ALLOWED
    /// NO LINT: entry functions are the correct pattern for Random access
    #[allow(lint(public_entry))]
    public entry fun flip_coin(r: &Random, ctx: &mut TxContext) {
        let _gen = sui::random::new_generator(r, ctx);
        // Use it internally
    }
    
    /// Private function with Random - ALLOWED
    /// NO LINT: private functions don't expose Random externally
    #[allow(unused_function)]
    fun internal_random_helper(_r: &Random): u64 {
        42
    }
    
    /// Package function with Random - ALLOWED
    /// NO LINT: package-internal functions don't expose Random externally
    public(package) fun package_random_helper(_r: &Random): u64 {
        42
    }
    
    /// Public function with RandomGenerator - ALLOWED
    /// NO LINT: RandomGenerator is safe to pass around (derived from Random)
    #[allow(lint(public_random))]
    public fun use_generator(_gen: &mut RandomGenerator): u64 {
        42
    }
    
    /// Public function with no Random - ALLOWED
    /// NO LINT: no Random parameter at all
    public fun regular_function(x: u64): u64 {
        x * 2
    }
    
    /// Custom Random type (not sui::random::Random) - ALLOWED
    /// NO LINT: this is a different type, not the framework Random
    public struct CustomRandom has key {
        id: sui::object::UID,
        seed: u64,
    }
    
    public fun use_custom_random(r: &CustomRandom): u64 {
        r.seed
    }
}

// =============================================================================
// SUPPRESSION CASES - Using move_clippy directives
// =============================================================================

module public_random_access_v2_pkg::suppression_cases {
    use sui::random::Random;
    
    /// Suppressed case - developer explicitly allows this pattern
    #[ext(move_clippy(allow(public_random_access_v2)))]
    public fun suppressed_random_access(_r: &Random): u64 {
        // NO LINT: suppressed by developer annotation
        42
    }
}
