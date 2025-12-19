//! Type classification utilities for Move types.
//!
//! These functions provide type-based detection using Move compiler's ability system,
//! eliminating the need for name-based heuristics. All classifications are based on
//! the type's abilities (key, store, copy, drop) which are compiler-verified.
//!
//! # Ability Patterns
//!
//! | Type Pattern | key | store | copy | drop | Use Case |
//! |--------------|-----|-------|------|------|----------|
//! | Capability   | ✓   | ✓     | ✗    | ✗    | Access control (AdminCap, TreasuryCap) |
//! | Hot Potato   | ✗   | ✗     | ✗    | ✗    | Flash loans, must be consumed |
//! | Resource     | ✓   | ✓     | ✗    | ?    | Valuable objects (Coin, NFT) |
//! | Event        | ✗   | ✗     | ✓    | ✓    | Emitted events |
//! | Config       | ✗   | ✓     | ✓    | ✓    | Configuration structs |

use move_compiler::expansion::ast::AbilitySet;
use move_compiler::naming::ast as N;
use move_compiler::parser::ast::Ability_;

// ============================================================================
// Core Ability Extraction
// ============================================================================

/// Extract abilities from a Move type.
/// Returns None for types without explicit abilities (e.g., primitives).
pub fn abilities_of_type(ty: &N::Type_) -> Option<AbilitySet> {
    match ty {
        N::Type_::Apply(abilities, _, _) => abilities.clone(),
        N::Type_::Ref(_, inner) => abilities_of_type(&inner.value),
        N::Type_::Param(tp) => Some(tp.abilities.clone()),
        _ => None,
    }
}

// ============================================================================
// Type-Based Classification (Zero Heuristics)
// ============================================================================

/// Capability: key + store, NO copy, NO drop
///
/// Capabilities are access control tokens that:
/// - Can be stored (store) and used as object IDs (key)
/// - Cannot be duplicated (no copy) - ensures single owner
/// - Cannot be silently discarded (no drop) - ensures explicit handling
///
/// Examples: AdminCap, TreasuryCap, UpgradeCap, OwnerCap
pub fn is_capability_type(abilities: &AbilitySet) -> bool {
    abilities.has_ability_(Ability_::Key)
        && abilities.has_ability_(Ability_::Store)
        && !abilities.has_ability_(Ability_::Copy)
        && !abilities.has_ability_(Ability_::Drop)
}

/// Check if a Type_ represents a capability
pub fn is_capability_type_from_ty(ty: &N::Type_) -> bool {
    abilities_of_type(ty).is_some_and(|a| is_capability_type(&a))
}

/// Hot Potato: NO abilities at all
///
/// Hot potatoes are structs that MUST be consumed - they cannot be:
/// - Stored (no store)
/// - Copied (no copy)
/// - Dropped (no drop)
/// - Used as object IDs (no key)
///
/// This pattern enforces that the struct must be explicitly handled,
/// commonly used for flash loan receipts.
///
/// Examples: FlashLoanReceipt, BorrowReceipt, SwapPromise
pub fn is_hot_potato_type(abilities: &AbilitySet) -> bool {
    !abilities.has_ability_(Ability_::Key)
        && !abilities.has_ability_(Ability_::Store)
        && !abilities.has_ability_(Ability_::Copy)
        && !abilities.has_ability_(Ability_::Drop)
}

/// Check if a Type_ represents a hot potato
pub fn is_hot_potato_type_from_ty(ty: &N::Type_) -> bool {
    abilities_of_type(ty).is_some_and(|a| is_hot_potato_type(&a))
}

/// Resource: key + store (valuable on-chain objects)
///
/// Resources are valuable objects that:
/// - Can be stored and used as object IDs
/// - May or may not be copyable/droppable
///
/// This is the broadest category for "valuable" types.
///
/// Examples: `Coin<T>`, NFT, Pool, Vault
pub fn is_resource_type(abilities: &AbilitySet) -> bool {
    abilities.has_ability_(Ability_::Key) && abilities.has_ability_(Ability_::Store)
}

/// Check if a Type_ represents a resource
pub fn is_resource_type_from_ty(ty: &N::Type_) -> bool {
    abilities_of_type(ty).is_some_and(|a| is_resource_type(&a))
}

/// Event: copy + drop only, NO key, NO store
///
/// Events are emitted via `event::emit` and should:
/// - Be copyable (copy) - can be read multiple times
/// - Be droppable (drop) - no cleanup needed
/// - NOT be stored (no store) - ephemeral
/// - NOT be objects (no key) - not on-chain objects
///
/// Examples: Transferred, PoolCreated, SwapExecuted
pub fn is_event_type(abilities: &AbilitySet) -> bool {
    abilities.has_ability_(Ability_::Copy)
        && abilities.has_ability_(Ability_::Drop)
        && !abilities.has_ability_(Ability_::Key)
        && !abilities.has_ability_(Ability_::Store)
}

/// Check if a Type_ represents an event
pub fn is_event_type_from_ty(ty: &N::Type_) -> bool {
    abilities_of_type(ty).is_some_and(|a| is_event_type(&a))
}

/// Config: copy + drop + store, NO key
///
/// Configuration structs that:
/// - Can be stored in other objects (store)
/// - Can be copied and dropped freely
/// - Are NOT standalone objects (no key)
///
/// Examples: PoolConfig, FeeConfig, Parameters
pub fn is_config_type(abilities: &AbilitySet) -> bool {
    abilities.has_ability_(Ability_::Copy)
        && abilities.has_ability_(Ability_::Drop)
        && abilities.has_ability_(Ability_::Store)
        && !abilities.has_ability_(Ability_::Key)
}

/// Check if a Type_ represents a config struct
pub fn is_config_type_from_ty(ty: &N::Type_) -> bool {
    abilities_of_type(ty).is_some_and(|a| is_config_type(&a))
}

// ============================================================================
// Legacy Compatibility (used by existing code)
// ============================================================================

/// Check if type has key + store (broad resource check)
pub fn is_key_store_type(ty: &N::Type_) -> bool {
    abilities_of_type(ty)
        .is_some_and(|a| a.has_ability_(Ability_::Key) && a.has_ability_(Ability_::Store))
}

/// Check if type has copy + drop
pub fn is_copy_drop_type(ty: &N::Type_) -> bool {
    abilities_of_type(ty)
        .is_some_and(|a| a.has_ability_(Ability_::Copy) && a.has_ability_(Ability_::Drop))
}

/// Check if type is event-like (copy + drop, no key)
/// Legacy name for is_event_type compatibility
pub fn is_event_like_type(ty: &N::Type_) -> bool {
    abilities_of_type(ty).is_some_and(|a| {
        a.has_ability_(Ability_::Copy)
            && a.has_ability_(Ability_::Drop)
            && !a.has_ability_(Ability_::Key)
    })
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Check if type has the drop ability
pub fn has_drop_ability(abilities: &AbilitySet) -> bool {
    abilities.has_ability_(Ability_::Drop)
}

/// Check if type has the copy ability
pub fn has_copy_ability(abilities: &AbilitySet) -> bool {
    abilities.has_ability_(Ability_::Copy)
}

/// Check if type has the key ability
pub fn has_key_ability(abilities: &AbilitySet) -> bool {
    abilities.has_ability_(Ability_::Key)
}

/// Check if type has the store ability
pub fn has_store_ability(abilities: &AbilitySet) -> bool {
    abilities.has_ability_(Ability_::Store)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use move_compiler::expansion::ast::AbilitySet;
    use move_compiler::parser::ast::Ability_;
    use move_ir_types::location::{Loc, Spanned};

    fn make_abilities(abilities: &[Ability_]) -> AbilitySet {
        let mut set = AbilitySet::empty();
        for ability in abilities {
            let spanned = Spanned {
                loc: Loc::invalid(),
                value: *ability,
            };
            let _ = set.add(spanned);
        }
        set
    }

    #[test]
    fn test_capability_type() {
        // Capability: key + store, no copy, no drop
        let cap = make_abilities(&[Ability_::Key, Ability_::Store]);
        assert!(is_capability_type(&cap));

        // Not capability: has copy
        let with_copy = make_abilities(&[Ability_::Key, Ability_::Store, Ability_::Copy]);
        assert!(!is_capability_type(&with_copy));

        // Not capability: has drop
        let with_drop = make_abilities(&[Ability_::Key, Ability_::Store, Ability_::Drop]);
        assert!(!is_capability_type(&with_drop));

        // Not capability: missing key
        let no_key = make_abilities(&[Ability_::Store]);
        assert!(!is_capability_type(&no_key));
    }

    #[test]
    fn test_hot_potato_type() {
        // Hot potato: no abilities
        let hot_potato = make_abilities(&[]);
        assert!(is_hot_potato_type(&hot_potato));

        // Not hot potato: has drop (the security bug we detect!)
        let with_drop = make_abilities(&[Ability_::Drop]);
        assert!(!is_hot_potato_type(&with_drop));

        // Not hot potato: has any ability
        let with_store = make_abilities(&[Ability_::Store]);
        assert!(!is_hot_potato_type(&with_store));
    }

    #[test]
    fn test_resource_type() {
        // Resource: key + store
        let resource = make_abilities(&[Ability_::Key, Ability_::Store]);
        assert!(is_resource_type(&resource));

        // Resource with drop is still a resource
        let with_drop = make_abilities(&[Ability_::Key, Ability_::Store, Ability_::Drop]);
        assert!(is_resource_type(&with_drop));

        // Not resource: missing key
        let no_key = make_abilities(&[Ability_::Store, Ability_::Copy, Ability_::Drop]);
        assert!(!is_resource_type(&no_key));
    }

    #[test]
    fn test_event_type() {
        // Event: copy + drop, no key, no store
        let event = make_abilities(&[Ability_::Copy, Ability_::Drop]);
        assert!(is_event_type(&event));

        // Not event: has store
        let with_store = make_abilities(&[Ability_::Copy, Ability_::Drop, Ability_::Store]);
        assert!(!is_event_type(&with_store));

        // Not event: has key
        let with_key = make_abilities(&[Ability_::Copy, Ability_::Drop, Ability_::Key]);
        assert!(!is_event_type(&with_key));
    }

    #[test]
    fn test_config_type() {
        // Config: copy + drop + store, no key
        let config = make_abilities(&[Ability_::Copy, Ability_::Drop, Ability_::Store]);
        assert!(is_config_type(&config));

        // Not config: has key (would be a resource)
        let with_key = make_abilities(&[
            Ability_::Copy,
            Ability_::Drop,
            Ability_::Store,
            Ability_::Key,
        ]);
        assert!(!is_config_type(&with_key));

        // Not config: missing store
        let no_store = make_abilities(&[Ability_::Copy, Ability_::Drop]);
        assert!(!is_config_type(&no_store));
    }
}
