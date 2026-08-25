mod build;
mod diagnostic;
mod model;
mod overlap;
mod predicate;
mod required_method;
mod selection;
mod validate;

#[cfg(test)]
mod tests;

pub use crate::type_relations::SubstitutionError;
#[cfg(test)]
use build::build_conformance_table;
pub(crate) use build::build_conformance_table_from_ids;
pub use build::{ConformanceBuildError, ConformanceInternalError, MissingConformanceMethods};
pub use diagnostic::ConformanceRule;
pub use model::{CheckedConformance, ConformanceMethod, ConformanceTable, MethodSelection};
pub(crate) use predicate::normalize_requirements;
pub(crate) use predicate::substitute_predicate;
pub use predicate::{CheckedPredicate, CheckedRequirement};
pub use required_method::{RequiredConformanceMethod, RequiredConformanceParameter};
pub(crate) use selection::{proves as proves_predicate, select_conformance};
