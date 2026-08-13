//! Structured semantic presentation shared by editor features.

mod callables;
mod locals;
mod symbols;
mod types;

pub(crate) use callables::{
    CallablePresentation, LiteralPresentation, associated_function_presentation,
    callable_signature_presentation, canonical_where_predicate_labels, drop_presentation,
    literal_presentation_with_substitutions, literal_signature_presentation,
    method_or_operator_presentation, method_presentation, method_presentation_with_substitutions,
    owner_type_expr, result_origin_labels, where_predicate_labels,
};
pub(crate) use locals::local_presentation;
pub(crate) use symbols::symbol_presentation_without_resolution;
pub(crate) use types::{
    generic_parameter_presentation, type_declaration_presentation, type_owner_presentation_label,
    type_reference_presentation,
};
