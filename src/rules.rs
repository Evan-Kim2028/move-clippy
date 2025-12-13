mod util;

pub mod patterns;

pub mod conventions;
pub mod modernization;
pub mod style;
pub mod test_quality;

// Conventions lints
pub use conventions::AdminCapPositionLint;

// Modernization lints
pub use modernization::{
    EqualityInAssertLint, ManualLoopIterationLint, ManualOptionCheckLint, ModernMethodSyntaxLint,
    ModernModuleSyntaxLint, PreferVectorMethodsLint, PublicMutTxContextLint,
    PureFunctionTransferLint, UnnecessaryPublicEntryLint, UnsafeArithmeticLint,
    WhileTrueToLoopLint,
};

// Style lints
pub use style::{
    AbilitiesOrderLint, ConstantNamingLint, DocCommentStyleLint, EmptyVectorLiteralLint,
    EventSuffixLint, ExplicitSelfAssignmentsLint, PreferToStringLint, RedundantSelfImportLint,
    TypedAbortCodeLint, UnneededReturnLint,
};

// Test quality lints
pub use test_quality::{MergeTestAttributesLint, RedundantTestPrefixLint, TestAbortCodeLint};
