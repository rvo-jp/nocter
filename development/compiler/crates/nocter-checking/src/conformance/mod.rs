mod build;
mod diagnostic;
mod model;
mod overlap;
mod predicate;
mod proof;
mod substitution;
mod validate;

#[cfg(test)]
mod tests;

pub use build::{ConformanceBuildError, ConformanceInternalError, build_conformance_table};
pub use diagnostic::ConformanceRule;
pub use model::{CheckedConformance, ConformanceMethod, ConformanceTable, MethodSelection};
pub use predicate::{CheckedPredicate, CheckedRequirement};
pub use substitution::SubstitutionError;
