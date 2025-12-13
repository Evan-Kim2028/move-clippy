mod util;

pub mod modernization;
pub mod style;
pub mod test_quality;

pub use modernization::{ModernMethodSyntaxLint, ModernModuleSyntaxLint, PreferVectorMethodsLint};
pub use style::{PreferToStringLint, RedundantSelfImportLint};
pub use test_quality::MergeTestAttributesLint;
