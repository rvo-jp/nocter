use super::*;

pub(in crate::typecheck::returns) fn method_receiver_is_borrow(
    method: &crate::resolve::MethodSignature,
) -> bool {
    method.receiver.mode != MethodReceiverMode::Owned
}

pub(in crate::typecheck::returns) fn borrow_return_provenance_for_borrowed_input(
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) -> Option<ValueProvenance> {
    let ty = expression_type(expression, resolved, environment);
    if type_contains_borrow_like(&ty, resolved) {
        return borrow_return_provenance_for_expression(
            expression,
            &ty,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        );
    }

    let Some(identifier) = expression_root_identifier(expression) else {
        return Some(ValueProvenance::scope(
            expression.span(),
            "temporary expression".to_string(),
        ));
    };
    if environment
        .get(&identifier.name)
        .is_some_and(|ty| type_contains_borrow_like(ty, resolved))
    {
        return borrow_return_provenance_for_identifier(
            identifier,
            resolved,
            environment,
            borrow_provenance,
        );
    }

    // A moved or forwarded value keeps the origin already recorded for its
    // binding. Treating the forwarding local as an additional storage owner
    // would make `from input` contracts fail merely because an implementation
    // gave the input a local name. A genuinely local value has no recorded
    // upstream provenance and still falls back to its lexical storage scope.
    borrow_return_provenance_for_identifier(identifier, resolved, environment, borrow_provenance)
        .or_else(|| borrow_return_provenance_for_local_storage(identifier, resolved))
}

pub(in crate::typecheck::returns) fn value_provenance_for_call_input(
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) -> Option<ValueProvenance> {
    let ty = expression_type(expression, resolved, environment);
    if type_contains_borrow_like(&ty, resolved) {
        return borrow_return_provenance_for_borrowed_input(
            expression,
            resolved,
            environment,
            borrow_provenance,
            summaries,
        );
    }

    borrow_return_provenance_for_expression(
        expression,
        &ty,
        resolved,
        environment,
        borrow_provenance,
        summaries,
    )
    .or(Some(ValueProvenance::Independent))
}

pub(in crate::typecheck::returns) fn borrow_return_provenance_for_identifier(
    identifier: &crate::ast::IdentifierExpr,
    resolved: &ResolveOutput,
    _environment: &TypeEnvironment,
    borrow_provenance: &ProvenanceEnvironment,
) -> Option<ValueProvenance> {
    let local_symbol = resolved.local_symbol_for_identifier(identifier)?;
    if let Some(provenance) = borrow_provenance.get(local_symbol.name_span) {
        return Some(provenance.clone());
    }

    if matches!(local_symbol.kind, LocalSymbolKind::Parameter) {
        return Some(ValueProvenance::input(InputId::declared_at(
            local_symbol.name_span,
        )));
    }

    None
}

pub(in crate::typecheck::returns) fn borrow_return_provenance_for_direct_borrow(
    expression: &Expr,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) -> Option<ValueProvenance> {
    let Expr::Borrow(borrow) = unwrap_group(expression) else {
        return None;
    };

    match unwrap_group(&borrow.expression) {
        Expr::Identifier(identifier) => {
            borrow_return_provenance_for_local_storage(identifier, resolved)
        }
        Expr::Member(member) => {
            let mut provenance = borrow_return_provenance_for_member(
                member,
                resolved,
                environment,
                borrow_provenance,
                summaries,
            );
            if let Some(identifier) = expression_root_identifier(&member.object) {
                let root_type = environment.get(&identifier.name);
                let root_provenance =
                    if root_type.is_some_and(|ty| type_contains_borrow_like(ty, resolved)) {
                        borrow_return_provenance_for_identifier(
                            identifier,
                            resolved,
                            environment,
                            borrow_provenance,
                        )
                    } else {
                        borrow_return_provenance_for_local_storage(identifier, resolved)
                    };
                merge_provenance(&mut provenance, root_provenance);
            } else {
                merge_provenance(
                    &mut provenance,
                    Some(ValueProvenance::scope(
                        member.span,
                        "temporary expression".to_string(),
                    )),
                );
            }
            provenance
        }
        Expr::Index(index)
            if type_contains_borrow_like(
                &expression_type(&index.object, resolved, environment),
                resolved,
            ) =>
        {
            borrow_return_provenance_for_index(
                index,
                resolved,
                environment,
                borrow_provenance,
                summaries,
            )
        }
        expression => Some(ValueProvenance::scope(
            expression.span(),
            "temporary expression".to_string(),
        )),
    }
}

pub(in crate::typecheck::returns) fn borrow_return_provenance_for_local_storage(
    identifier: &crate::ast::IdentifierExpr,
    resolved: &ResolveOutput,
) -> Option<ValueProvenance> {
    let symbol = resolved.local_symbol_for_identifier(identifier)?;
    let source = match symbol.kind {
        LocalSymbolKind::Parameter => format!("parameter `{}`", identifier.name),
        LocalSymbolKind::Binding(_) => format!("local binding `{}`", identifier.name),
        LocalSymbolKind::Region => format!("region binding `{}`", identifier.name),
        LocalSymbolKind::PatternPayload => format!("payload binding `{}`", identifier.name),
        LocalSymbolKind::CatchError => format!("catch binding `{}`", identifier.name),
        LocalSymbolKind::ForRange => format!("for-range binding `{}`", identifier.name),
        LocalSymbolKind::CollectionFor => {
            format!("collection loop binding `{}`", identifier.name)
        }
        LocalSymbolKind::LiteralPackFor => {
            format!("literal-pack loop binding `{}`", identifier.name)
        }
        LocalSymbolKind::LiteralCapture => format!("literal pack `{}`", identifier.name),
        LocalSymbolKind::ClosureCapture(_) => format!("closure capture `{}`", identifier.name),
    };

    Some(ValueProvenance::scope(symbol.name_span, source))
}
