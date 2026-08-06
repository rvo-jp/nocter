//! Structured semantic presentation shared by editor features.

mod anchors;
mod ast_declarations;
mod callables;
mod locals;
mod semantic_details;
mod symbols;
mod types;

pub(crate) use anchors::CallableDeclarationIndex;
pub(crate) use ast_declarations::{
    ast_drop_presentation, ast_enum_presentation, ast_function_presentation,
    ast_interface_presentation, ast_literal_presentation, ast_method_presentation,
    ast_parameter_labels, ast_primitive_presentation, ast_struct_presentation,
    ast_type_alias_presentation,
};
pub(crate) use callables::{
    CallablePresentation, LiteralPresentation, associated_function_presentation,
    callable_signature_presentation, drop_presentation, literal_presentation_with_substitutions,
    literal_signature_presentation, method_presentation, method_presentation_with_substitutions,
    owner_type_expr, result_origin_labels,
};
pub(crate) use locals::local_presentation;
pub(crate) use semantic_details::{
    AllocationEffectPresentation, ResultProvenancePresentation, SemanticDetail,
    semantic_details_for_callable, semantic_details_for_callable_result,
};
pub(crate) use symbols::symbol_presentation_without_resolution;
pub(crate) use types::{
    generic_parameter_presentation, type_declaration_presentation, type_owner_presentation_label,
    type_reference_presentation,
};
