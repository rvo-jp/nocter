mod build;
mod diagnostic;
mod model;
mod overlap;
mod predicate;
mod selection;
mod validate;

#[cfg(test)]
mod tests;

pub use crate::type_relations::SubstitutionError;
pub use build::{ConformanceBuildError, ConformanceInternalError, build_conformance_table};
pub use diagnostic::ConformanceRule;
pub use model::{CheckedConformance, ConformanceMethod, ConformanceTable, MethodSelection};
pub(crate) use predicate::normalize_requirements;
pub(crate) use predicate::substitute_predicate;
pub use predicate::{CheckedPredicate, CheckedRequirement};
pub(crate) use selection::{proves as proves_predicate, select_conformance};
