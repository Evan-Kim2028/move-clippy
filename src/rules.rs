mod util;

pub mod patterns;

pub mod conventions;
pub mod modernization;
pub mod security;
pub mod style;
pub mod test_quality;

// Conventions lints
pub use conventions::AdminCapPositionLint;

// Modernization lints
pub use modernization::{
    EqualityInAssertLint, ManualLoopIterationLint, ManualOptionCheckLint, ModernMethodSyntaxLint,
    ModernModuleSyntaxLint, PreferVectorMethodsLint,
};
// REMOVED from modernization:
// - WhileTrueToLoopLint, UnnecessaryPublicEntryLint, PublicMutTxContextLint (compiler-redundant)
// - PureFunctionTransferLint, UnsafeArithmeticLint (experimental, questionable value)

// Security lints (audit-backed)
pub use security::{
    FreshAddressReuseLint, SuggestBalancedReceiptLint, SuggestCapabilityPatternLint,
    SuggestCountedCapabilityLint, SuggestSequencedWitnessLint, SuspiciousOverflowCheckLint,
};
// REMOVED deprecated/superseded/obvious lints:
// - StaleOraclePriceLint, SingleStepOwnershipTransferLint, UncheckedCoinSplitLint
// - MissingWitnessDropLint, PublicRandomAccessLint, IgnoredBooleanReturnLint
// - UncheckedWithdrawalLint, CapabilityLeakLint, DigestAsRandomnessLint
// - OtwPatternViolationLint (duplicates Sui Verifier)
// - DestroyZeroUncheckedLint, DivideByZeroLiteralLint (obvious/trivial)

// Style lints
pub use style::{
    AbilitiesOrderLint, ConstantNamingLint, DocCommentStyleLint, EmptyVectorLiteralLint,
    ErrorConstNamingLint, ExplicitSelfAssignmentsLint, PreferToStringLint, RedundantSelfImportLint,
    TypedAbortCodeLint, UnneededReturnLint,
};
// REMOVED: EventSuffixLint (not backed by Move Book)

// Test quality lints
pub use test_quality::{MergeTestAttributesLint, RedundantTestPrefixLint, TestAbortCodeLint};
