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
        ty if ty.integer_type().is_some() => {
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
        AbiType::Borrow => {
            let Type::Borrow {
                is_readwrite,
                inner,
            } = context.ir_type_for_type_expr(&ty).ok_or_else(|| {
                unsupported_binding_diagnostic(
                    "IR cannot recover a specialized pointer-take borrow type",
                )
            })?
            else {
                return Err(unsupported_binding_diagnostic(
                    "IR pointer-take ABI and borrow type disagree",
                ));
            };
            let destination = context.next_usize_local_location()?;
            let mut temporaries = TemporaryAllocator::new(context)?;
            let instructions = lower_take_value_at_ptr_primitive_call(
                call,
                PointerTakeDestination::Usize(destination),
                context,
                &mut temporaries,
            )?;
            context.define_borrow_local(statement.name.clone(), is_readwrite, *inner);
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
        AbiType::Outcome { layout } => {
            let Some((_root_source, resolved)) = context.resolved_calls() else {
                return Err(unsupported_binding_diagnostic(
                    "IR cannot lower a specialized pointer-take outcome without resolved type information",
                ));
            };
            let shape = outcome_shape_with_resolver(&ty, resolved, |source| {
                context.resolved_source(source)
            });
            let payload_abi =
                abi_value_from_type_expr_with_resolver(&shape.payload, resolved, |source| {
                    context.resolved_source(source)
                })
                .map_err(|_| {
                    unsupported_binding_diagnostic(
                        "IR cannot lay out a specialized pointer-take outcome payload",
                    )
                })?;
            let storage = shape.storage_layout(payload_abi.layout).ok_or_else(|| {
                unsupported_binding_diagnostic(
                    "IR cannot represent a specialized pointer-take outcome shape",
                )
            })?;
            if storage.layout != layout {
                return Err(unsupported_binding_diagnostic(
                    "IR pointer-take outcome storage disagrees with its ABI layout",
                ));
            }
            let payload_type =
                return_type_from_type_expr_with_resolver(&shape.payload, resolved, |source| {
                    context.resolved_source(source)
                })
                .ok_or_else(|| {
                    unsupported_binding_diagnostic(
                        "IR cannot represent a specialized pointer-take outcome payload",
                    )
                })?;
            let is_copy = matches!(
                payload_type,
                Type::I32
                    | Type::U8
                    | Type::Usize
                    | Type::Bool
                    | Type::Str
                    | Type::Slice { .. }
                    | Type::Borrow { .. }
            ) || type_expr_is_copy_aggregate_value_with_resolver(
                &shape.payload,
                resolved,
                |source| context.resolved_source(source),
            );
            let drop_kind = context
                .aggregate_drop_for_type_expr(&shape.payload)
                .map(|payload| {
                    AggregateDrop::Outcome(OutcomeDrop {
                        storage: storage.clone(),
                        payload: Box::new(payload),
                    })
                });
            let slot_index = context.reserve_aggregate_slot_index();
            let mut instructions = vec![Instruction::ReserveAggregateSlot { slot_index, layout }];
            let mut temporaries = TemporaryAllocator::new(context)?;
            instructions.extend(lower_take_value_at_ptr_primitive_call(
                call,
                PointerTakeDestination::Aggregate {
                    location: AggregateLocation::Slot(slot_index),
                    layout,
                },
                context,
                &mut temporaries,
            )?);
            context.define_outcome_local_at_slot(
                statement.name.clone(),
                slot_index,
                storage,
                payload_type,
                is_copy,
                drop_kind,
            );
            Ok(Some(instructions))
        }
        AbiType::SliceView
        | AbiType::I8
        | AbiType::I16
        | AbiType::I64
        | AbiType::U16
        | AbiType::U32
        | AbiType::U64
        | AbiType::Isize => Ok(None),
    }
}
