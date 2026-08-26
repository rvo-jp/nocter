mod concrete;
mod pattern;
mod structure;
mod substitution;
mod unify;

pub(crate) use structure::{map_type_children, visit_type_children};

pub use concrete::is_concrete_type;
pub(crate) use pattern::{match_type_pattern, type_patterns_overlap};
pub use substitution::SubstitutionError;
pub use substitution::TypeSubstitution;
pub(crate) use unify::{
    GenericBindings, TypeUnificationError, collect_generic_parameters, unify_type_pairs,
};
