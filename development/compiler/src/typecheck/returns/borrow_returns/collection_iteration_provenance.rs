//! Provenance carried by collection-iteration bindings.
//!
//! Collection `for` is protocol syntax rather than an AST call expression, so
//! it must instantiate the resolved conversion and `next` summaries explicitly.
//! Keeping that translation here prevents return and mutation analyses from
//! treating yielded generic values as storage-independent locals.

use super::*;
use crate::ast::CollectionForStmt;

pub(in crate::typecheck::returns) fn collection_iteration_item_flow(
    statement: &CollectionForStmt,
    resolved: &ResolveOutput,
    environment: &TypeEnvironment,
    borrow_provenance: &ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) -> (Type, Option<ValueProvenance>) {
    let Ok(plan) =
        crate::typecheck::iteration::resolve_collection_iteration(statement, resolved, environment)
    else {
        return (Type::Unknown, None);
    };
    let item_type = plan.item_type.clone();
    let source = value_provenance_for_call_input(
        &statement.source,
        resolved,
        environment,
        borrow_provenance,
        summaries,
    )
    .unwrap_or(ValueProvenance::Independent);
    let iterator = plan
        .conversion
        .as_ref()
        .and_then(|conversion| {
            instantiate_iteration_method_result(
                conversion.declaration,
                &source,
                resolved,
                borrow_provenance,
                summaries,
            )
        })
        .unwrap_or(source);
    let item = instantiate_iteration_method_result(
        plan.step.declaration,
        &iterator,
        resolved,
        borrow_provenance,
        summaries,
    )
    .and_then(|provenance| provenance.success_provenance());
    (item_type, item)
}

fn instantiate_iteration_method_result(
    declaration: ByteSpan,
    receiver: &ValueProvenance,
    resolved: &ResolveOutput,
    borrow_provenance: &ProvenanceEnvironment,
    summaries: &CallableProvenanceSummaries,
) -> Option<ValueProvenance> {
    let summary = summaries.result(CallableId::for_declaration(resolved, declaration)?)?;
    let method = resolved.method_signature_by_name_span(declaration)?;
    let receiver_input = InputId::resolved_at(resolved, method.receiver.name_span);
    instantiate_provenance_summary(summary, &mut |origin| match origin {
        StorageOrigin::Static => Some(ValueProvenance::static_storage()),
        StorageOrigin::CurrentAllocationContext => {
            Some(borrow_provenance.current_allocation_context_provenance())
        }
        StorageOrigin::Input(input) if *input == receiver_input => Some(receiver.clone()),
        StorageOrigin::Input(_) => Some(ValueProvenance::unknown()),
        StorageOrigin::InputWithCurrentFallback(_) => {
            unreachable!("conditional inputs are instantiated before origin mapping")
        }
        StorageOrigin::Allocated(_) => unreachable!("summary instantiation unwraps allocations"),
        StorageOrigin::Scope { .. } | StorageOrigin::Region { .. } | StorageOrigin::Unknown => {
            Some(ValueProvenance::unknown())
        }
    })
}

pub(in crate::typecheck::returns) fn define_collection_iteration_item_provenance(
    statement: &CollectionForStmt,
    item_type: &Type,
    item_provenance: Option<ValueProvenance>,
    resolved: &ResolveOutput,
    borrow_provenance: &mut ProvenanceEnvironment,
) {
    let contains_storage = type_may_carry_result_provenance(item_type, resolved)
        || item_provenance
            .as_ref()
            .is_some_and(ValueProvenance::has_storage_dependency);
    borrow_provenance.define_binding_at(
        resolved,
        statement.name_span,
        contains_storage,
        item_provenance,
    );
}
