mod build;
mod diagnostic;
mod model;
mod overlap;
mod predicate;
mod proof;
mod validate;

#[cfg(test)]
mod tests;

pub use crate::type_relations::SubstitutionError;
pub use build::{ConformanceBuildError, ConformanceInternalError, build_conformance_table};
pub use diagnostic::ConformanceRule;
pub use model::{CheckedConformance, ConformanceMethod, ConformanceTable, MethodSelection};
pub use predicate::{CheckedPredicate, CheckedRequirement};
