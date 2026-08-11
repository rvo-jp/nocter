//! Drop-aware replacement of aggregate slice elements.
//!
//! Scalar stores and copy-aggregate stores have direct IR operations. A
//! move-only aggregate replacement additionally has to stage the replacement,
//! bounds-check the destination, destroy the old value, and transfer the new
//! bytes without registering either staging slot as a lexical owner.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn lower_aggregate_slice_index_replacement(
    target: &IndexExpr,
    value: &Expr,
    destination: SliceLocation,
    index: UsizeValue,
    mut instructions: Vec<Instruction>,
    context: &LoweringContext,
    temporaries: &mut TemporaryAllocator,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let element_ty = slice_index_element_type_expr(target, context)
        .ok_or_else(unsupported_assignment_diagnostic)?;
    let (_root_source, resolved) = context
        .resolved_calls()
        .ok_or_else(unsupported_assignment_diagnostic)?;
    if type_expr_is_copy_aggregate_value_with_resolver(&element_ty, resolved, |source| {
        context.resolved_source(source)
    }) {
        return lower_copy_aggregate_slice_index_assignment(
            target,
            value,
            destination,
            index,
            instructions,
            context,
            temporaries,
        );
    }

    let abi = context
        .abi_value_for_type_expr(&element_ty)
        .ok_or_else(unsupported_assignment_diagnostic)?;
    if !supported_aggregate_copy_layout(abi.layout) {
        return Err(unsupported_assignment_diagnostic());
    }
    let drop_kind = context.aggregate_drop_for_type_expr(&element_ty);
    let index = materialize_slice_aggregate_index(&mut instructions, index, temporaries)?;

    let replacement_slot = temporaries.next_aggregate_slot();
    instructions.push(Instruction::ReserveAggregateSlot {
        slot_index: replacement_slot,
        layout: abi.layout,
    });
    instructions.extend(lower_aggregate_assignment_to_slot(
        replacement_slot,
        abi.layout,
        Some(&element_ty),
        value,
        context,
    )?);

    // Copying the old element to a private slot performs the same checked
    // projection as every other aggregate slice read. It therefore traps
    // before destruction or overwrite when the index is out of bounds.
    let old_slot = temporaries.next_aggregate_slot();
    instructions.push(Instruction::ReserveAggregateSlot {
        slot_index: old_slot,
        layout: abi.layout,
    });
    instructions.push(Instruction::CopySliceElementToAggregate {
        destination: AggregateLocation::Slot(old_slot),
        source: destination,
        index,
        layout: abi.layout,
    });
    if let Some(drop_kind) = drop_kind {
        instructions.extend(lower_aggregate_drop_instructions(
            "indexed element",
            old_slot,
            abi.layout,
            &drop_kind,
            context,
        )?);
    }
    instructions.push(Instruction::CopyAggregateToSliceElement {
        destination,
        index,
        source: AggregateLocation::Slot(replacement_slot),
        layout: abi.layout,
    });
    Ok(instructions)
}
