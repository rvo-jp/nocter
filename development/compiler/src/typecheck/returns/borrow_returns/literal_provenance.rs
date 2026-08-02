use super::*;

/// Instantiates a literal body's callable summary at the expression site.
///
/// Literal parameters are not ordinary ABI arguments. The string parameter is
/// backed by the source literal's static bytes, while an ephemeral sequence
/// capture is forbidden from escaping its body. `using` replaces only the
/// declaration-relative current allocation context.
pub(in crate::typecheck::returns) fn borrow_return_provenance_for_typed_literal(
    span: ByteSpan,
    using: Option<&crate::ast::LiteralContextOverride>,
    has_static_string_parameter: bool,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) -> Option<ValueProvenance> {
    let resolution = resolved.literal_resolution(span)?;
    let signature = resolved.literal_signature(resolution)?;
    let summary = summaries.result(CallableId::declared_at(resolution.literal_declaration_span))?;

    instantiate_provenance_summary(summary, &mut |origin| match origin {
        StorageOrigin::Static => Some(ValueProvenance::static_storage()),
        StorageOrigin::CurrentAllocationContext => match using {
            Some(context) => value_provenance_for_call_input(
                &context.allocator,
                resolved,
                environment,
                borrow_provenance,
                summaries,
            )
            .or_else(|| Some(ValueProvenance::unknown())),
            None => Some(borrow_provenance.current_allocation_context_provenance()),
        },
        StorageOrigin::Input(source)
            if has_static_string_parameter
                && signature
                    .parameters
                    .iter()
                    .any(|parameter| InputId::declared_at(parameter.name_span) == *source) =>
        {
            Some(ValueProvenance::static_storage())
        }
        StorageOrigin::Input(_)
        | StorageOrigin::Scope { .. }
        | StorageOrigin::Region { .. }
        | StorageOrigin::Unknown => Some(ValueProvenance::unknown()),
    })
}
