//! Structured semantic presentation shared by editor features.

mod anchors;
mod callables;
mod locals;
mod semantic_details;
mod types;

pub(crate) use anchors::CallableDeclarationIndex;
pub(crate) use callables::{
    CallablePresentation, LiteralPresentation, associated_function_presentation,
    callable_signature_presentation, drop_presentation, literal_presentation_with_substitutions,
    literal_signature_presentation, method_presentation, owner_type_expr, result_origin_labels,
};
pub(crate) use locals::local_presentation;
pub(crate) use semantic_details::{
    AllocationEffectPresentation, ResultProvenancePresentation, SemanticDetail,
    semantic_details_for_callable,
};
pub(crate) use types::{
    generic_parameter_presentation, type_declaration_presentation, type_owner_presentation_label,
    type_reference_presentation,
};
