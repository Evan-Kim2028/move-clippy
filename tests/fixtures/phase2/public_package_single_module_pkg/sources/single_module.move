/// Test fixture for public_package_single_module lint.
/// 
/// This lint detects `public(package)` visibility in single-module packages
/// where it's semantically equivalent to `public` but suggests internal-only use.
///
/// Tier 1 (Zero FP): State space is visibility × module_count, fully deterministic.

module public_package_single_module_pkg::only_module {
    
    // ==========================================================================
    // POSITIVE CASES - Should trigger public_package_single_module
    // ==========================================================================

    /// public(package) in single-module package is redundant
    /// SHOULD FIRE: No other module can call this anyway
    public(package) fun package_internal_add(a: u64, b: u64): u64 {
        a + b
    }

    /// Another public(package) function - should fire
    /// SHOULD FIRE: Redundant visibility modifier
    public(package) fun package_internal_multiply(a: u64, b: u64): u64 {
        a * b
    }

    /// public(package) entry function - still redundant
    /// SHOULD FIRE: Entry doesn't change the visibility semantics
    public(package) entry fun package_internal_entry(value: u64) {
        let _ = value;
    }

    // ==========================================================================
    // NEGATIVE CASES - Should NOT trigger public_package_single_module
    // ==========================================================================

    /// Regular public function - fine
    public fun public_add(a: u64, b: u64): u64 {
        a + b
    }

    /// Private function - fine
    fun private_helper(x: u64): u64 {
        x * 2
    }

    /// Public entry function - fine
    public entry fun public_entry_point(value: u64) {
        let _ = value;
    }
}
