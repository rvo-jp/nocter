use super::*;

/// Instantiates a literal body's callable summary at the expression site.
///
/// Literal inputs are not ordinary ABI arguments. The string parameter is
/// backed by the source literal's static bytes, while a sequence capture maps
/// its declaration-identity origin to the supplied element provenances.
/// `using` replaces only the declaration-relative current allocation context.
pub(in crate::typecheck::returns) fn borrow_return_provenance_for_typed_literal(
    span: ByteSpan,
    using: Option<&crate::ast::LiteralContextOverride>,
    has_static_string_parameter: bool,
    sequence_elements: Option<&[Expr]>,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) -> Option<ValueProvenance> {
    let resolution = resolved.literal_resolution(span)?;
    let signature = resolved.literal_signature(resolution)?;
    let summary = summaries.result(CallableId::for_declaration(
        resolved,
        resolution.literal_declaration_span,
    )?)?;
    let sequence_input = sequence_elements.map(|elements| {
        sequence_pack_provenance(
            elements,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        )
    });
    let capture_input = signature
        .capture
        .as_ref()
        .map(|capture| InputId::declared_at(capture.name_span));

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
        StorageOrigin::Input(source) if capture_input.as_ref() == Some(source) => sequence_input
            .clone()
            .or(Some(ValueProvenance::Independent)),
        StorageOrigin::Input(_)
        | StorageOrigin::InputWithCurrentFallback(_)
        | StorageOrigin::Allocated(_)
        | StorageOrigin::Scope { .. }
        | StorageOrigin::Region { .. }
        | StorageOrigin::Unknown => Some(ValueProvenance::unknown()),
    })
}

fn sequence_pack_provenance(
    sequence_elements: &[Expr],
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) -> ValueProvenance {
    let mut elements = BTreeMap::new();
    for (index, element) in sequence_elements.iter().enumerate() {
        let Some(spread) = crate::typecheck::sequence_spread(element) else {
            let ty = expression_type(element, resolved, environment);
            if let Some(provenance) = borrow_return_provenance_for_expression(
                element,
                &ty,
                resolved,
                environment,
                borrow_provenance,
                summaries,
            ) {
                elements.insert(index, provenance);
            }
            continue;
        };
        let Ok(plan) =
            crate::typecheck::iteration::resolve_sequence_spread(spread, resolved, environment)
        else {
            continue;
        };
        if plan.mode == crate::typecheck::iteration::SequenceSpreadMode::Copy
            && !type_contains_borrow_like(&plan.pack_item_type, resolved)
        {
            continue;
        }
        let source = match spread.operand.without_groups() {
            Expr::Borrow(borrow) => borrow.expression.as_ref(),
            Expr::Unary(unary) if unary.operator == crate::ast::UnaryOperator::Move => {
                unary.operand.as_ref()
            }
            source => source,
        };
        let ty = expression_type(source, resolved, environment);
        if let Some(provenance) = borrow_return_provenance_for_expression(
            source,
            &ty,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        ) {
            elements.insert(index, provenance);
        }
    }
    if elements.is_empty() {
        ValueProvenance::Independent
    } else {
        ValueProvenance::Aggregate {
            fallback: None,
            fields: BTreeMap::new(),
            elements,
        }
    }
}
