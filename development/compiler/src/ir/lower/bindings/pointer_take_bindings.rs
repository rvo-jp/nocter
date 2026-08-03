use super::*;
use crate::ir::lower::expressions::{
    PointerTakeDestination, lower_take_value_at_ptr_primitive_call,
    primitive_take_value_at_ptr_call,
};

/// Lowers a concrete specialization of `let value: T = take_value_at_ptr(...)`.
///
/// Generic collection algorithms need the same local representation and drop
/// tracking as a non-generic binding after `T` has been specialized. Keeping
/// this at the binding boundary avoids collection-name or caller-name checks.
pub(super) fn lower_pointer_take_binding(
    statement: &BindingStmt,
    context: &mut LoweringContext,
) -> Result<Option<Vec<Instruction>>, Vec<Diagnostic>> {
    let Expr::Call(call) = unwrap_group(&statement.initializer) else {
        return Ok(None);
    };
    if !primitive_take_value_at_ptr_call(call, context) {
        return Ok(None);
    }
    let Some(annotation) = statement
        .ty
        .clone()
        .or_else(|| context.binding_type_expr(statement.name_span))
    else {
        return Ok(None);
    };
    let ty = context.specialize_type_expr(&annotation);
    let Some(value) = context.abi_value_for_type_expr(&ty) else {
        return Ok(None);
    };

    match value.ty {
        AbiType::I32 => {
            let destination = context.next_i32_local_location()?;
            let mut temporaries = TemporaryAllocator::new(context)?;
            let instructions = lower_take_value_at_ptr_primitive_call(
                call,
                PointerTakeDestination::I32(destination),
                context,
                &mut temporaries,
            )?;
            context.define_i32_local(statement.name.clone());
            Ok(Some(instructions))
        }
        AbiType::U8 => {
            let destination = context.next_u8_local_location()?;
            let mut temporaries = TemporaryAllocator::new(context)?;
            let instructions = lower_take_value_at_ptr_primitive_call(
                call,
                PointerTakeDestination::U8(destination),
                context,
                &mut temporaries,
            )?;
            context.define_u8_local(statement.name.clone());
            Ok(Some(instructions))
        }
        AbiType::Usize | AbiType::Pointer => {
            let destination = context.next_usize_local_location()?;
            let mut temporaries = TemporaryAllocator::new(context)?;
            let instructions = lower_take_value_at_ptr_primitive_call(
                call,
                PointerTakeDestination::Usize(destination),
                context,
                &mut temporaries,
            )?;
            context.define_usize_local(statement.name.clone());
            Ok(Some(instructions))
        }
        AbiType::Bool => {
            let destination = context.next_bool_local_location()?;
            let mut temporaries = TemporaryAllocator::new(context)?;
            let instructions = lower_take_value_at_ptr_primitive_call(
                call,
                PointerTakeDestination::Bool(destination),
                context,
                &mut temporaries,
            )?;
            context.define_bool_local(statement.name.clone());
            Ok(Some(instructions))
        }
        AbiType::StrView => {
            let destination = context.next_str_local_location()?;
            let mut temporaries = TemporaryAllocator::new(context)?;
            let instructions = lower_take_value_at_ptr_primitive_call(
                call,
                PointerTakeDestination::Str(destination),
                context,
                &mut temporaries,
            )?;
            context.define_str_local(statement.name.clone());
            Ok(Some(instructions))
        }
        AbiType::Struct(_) | AbiType::Array { .. } | AbiType::Enum(_) => {
            let Some((root_source, resolved)) = context.resolved_calls() else {
                return Err(unsupported_binding_diagnostic(
                    "IR cannot lower a specialized pointer-take aggregate without resolved type information",
                ));
            };
            validate_aggregate_binding_layout(value.layout)?;
            let is_copy =
                type_expr_is_copy_aggregate_value_with_resolver(&ty, resolved, |source| {
                    context.resolved_source(source)
                });
            let drop_kind = context.aggregate_drop_for_type_expr(&ty);
            let fields = aggregate_fields_from_type_expr_with_resolver(
                &ty,
                root_source,
                resolved,
                |source| context.resolved_source(source),
            )
            .unwrap_or_default();
            let slot_index = context.define_aggregate_local(
                statement.name.clone(),
                value.layout,
                is_copy,
                drop_kind,
                fields,
            );
            let mut instructions = vec![Instruction::ReserveAggregateSlot {
                slot_index,
                layout: value.layout,
            }];
            let mut temporaries = TemporaryAllocator::new(context)?;
            instructions.extend(lower_take_value_at_ptr_primitive_call(
                call,
                PointerTakeDestination::Aggregate {
                    location: AggregateLocation::Slot(slot_index),
                    layout: value.layout,
                },
                context,
                &mut temporaries,
            )?);
            context.mark_aggregate_local_initialized(&statement.name);
            Ok(Some(instructions))
        }
        AbiType::SliceView
        | AbiType::Borrow
        | AbiType::I8
        | AbiType::I16
        | AbiType::I64
        | AbiType::U16
        | AbiType::U32
        | AbiType::U64
        | AbiType::Isize
        | AbiType::Outcome { .. } => Ok(None),
    }
}
