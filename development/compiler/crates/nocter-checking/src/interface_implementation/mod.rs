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
use build::build_interface_implementation_table;
pub(crate) use build::build_interface_implementation_table_from_ids;
pub use build::{
    InterfaceImplementationBuildError, InterfaceImplementationInternalError,
    MissingInterfaceImplementationMethods,
};
pub use diagnostic::InterfaceImplementationRule;
pub use model::{
    CheckedInterfaceImplementation, InterfaceImplementationMethod, InterfaceImplementationTable,
    MethodSelection,
};
pub(crate) use predicate::normalize_requirements;
pub(crate) use predicate::substitute_predicate;
pub use predicate::{CheckedPredicate, CheckedRequirement};
pub use required_method::{
    RequiredInterfaceImplementationMethod, RequiredInterfaceImplementationParameter,
};
pub(crate) use selection::{proves as proves_predicate, select_interface_implementation};
