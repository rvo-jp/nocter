//! Projection of semantic MIR destruction plans into machine IR.

use super::{
    BackendContext, aggregate_local_abi_value, aggregate_location, invalid_mir_diagnostics,
};
use crate::abi::{AbiType, layout_struct};
use crate::diagnostics::Diagnostic;
use crate::ir::{AggregateLocation, BorrowArgument, BorrowSource, Instruction, ScalarArgument};
use crate::mir::{DropPlan, DropPlanId, Place};

pub(super) fn lower_drop(
    context: &BackendContext<'_>,
    place: Place,
    plan: DropPlanId,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let location = aggregate_location(&place, context)?;
    let ty = context.body.locals[place.local.index()].ty;
    lower_plan(context, location, 0, ty, plan)
}

fn lower_plan(
    context: &BackendContext<'_>,
    location: AggregateLocation,
    base_offset: u32,
    ty: crate::semantic::TyId,
    plan: DropPlanId,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let plan = context
        .body
        .drop_plans
        .get(plan.index())
        .ok_or_else(|| invalid_mir_diagnostics("drop references a missing semantic plan"))?;
    match plan {
        DropPlan::Noop => Ok(Vec::new()),
        DropPlan::Direct { destructor } => Ok(vec![direct_drop(
            context,
            location,
            base_offset,
            *destructor,
        )?]),
        DropPlan::Struct { destructor, fields } => {
            let value = aggregate_local_abi_value(ty, context)?;
            let AbiType::Struct(abi_fields) = value.ty else {
                return Err(invalid_mir_diagnostics(
                    "struct drop plan does not describe struct storage",
                ));
            };
            let layout = layout_struct(&abi_fields)
                .map_err(|error| invalid_mir_diagnostics(format!("{error:?}")))?;
            let mut instructions = Vec::new();
            if let Some(destructor) = destructor {
                instructions.push(direct_drop(context, location, base_offset, *destructor)?);
            }
            for field in fields.iter().rev() {
                let offset = layout
                    .fields
                    .get(field.index)
                    .and_then(|field| u32::try_from(field.offset).ok())
                    .and_then(|offset| base_offset.checked_add(offset))
                    .ok_or_else(|| invalid_mir_diagnostics("drop field offset is invalid"))?;
                instructions.extend(lower_plan(context, location, offset, field.ty, field.plan)?);
            }
            Ok(instructions)
        }
        DropPlan::Array {
            length,
            element_ty,
            element,
        } => {
            let value = aggregate_local_abi_value(ty, context)?;
            let AbiType::Array {
                element: abi_element,
                ..
            } = value.ty
            else {
                return Err(invalid_mir_diagnostics(
                    "array drop plan does not describe array storage",
                ));
            };
            let stride = crate::abi::array_element_stride(&abi_element)
                .map_err(|error| invalid_mir_diagnostics(format!("{error:?}")))?;
            let mut instructions = Vec::new();
            for index in (0..*length).rev() {
                let offset = index
                    .checked_mul(stride)
                    .and_then(|offset| u64::from(base_offset).checked_add(offset))
                    .and_then(|offset| u32::try_from(offset).ok())
                    .ok_or_else(|| invalid_mir_diagnostics("drop array offset is invalid"))?;
                instructions.extend(lower_plan(
                    context,
                    location,
                    offset,
                    *element_ty,
                    *element,
                )?);
            }
            Ok(instructions)
        }
    }
}

fn direct_drop(
    context: &BackendContext<'_>,
    location: AggregateLocation,
    offset: u32,
    destructor: crate::semantic::DefId,
) -> Result<Instruction, Vec<Diagnostic>> {
    let (target, _) = super::lower_call_target(
        destructor,
        context.resolved,
        context.function_names,
        context.root_source,
    )?;
    let source = match (location, offset) {
        (AggregateLocation::Slot(slot_index), 0) => BorrowSource::AggregateSlot(slot_index),
        (AggregateLocation::Slot(slot_index), offset) => {
            BorrowSource::AggregateSlotField { slot_index, offset }
        }
        (AggregateLocation::Parameter(parameter_index), 0) => {
            BorrowSource::AggregateParameter(parameter_index)
        }
        (AggregateLocation::Parameter(parameter_index), offset) => {
            BorrowSource::AggregateParameterField {
                parameter_index,
                offset,
            }
        }
        _ => {
            return Err(invalid_mir_diagnostics(
                "semantic drop place has no addressable aggregate storage",
            ));
        }
    };
    Ok(Instruction::CallVoid {
        target,
        arguments: vec![ScalarArgument::Borrow(BorrowArgument { source })],
    })
}
