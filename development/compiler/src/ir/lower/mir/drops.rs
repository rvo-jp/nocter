//! Projection of semantic MIR destruction plans into machine IR.

use super::{BackendContext, invalid_mir_diagnostics};
use crate::abi::{AbiType, layout_struct};
use crate::diagnostics::Diagnostic;
use crate::ir::{
    AggregateIndex, AggregateLocation, BoolValue, BorrowArgument, BorrowSource,
    I32ComparisonOperator, I32Value, Instruction, ScalarArgument, SliceElementIndex, U8Location,
    U8Value, UsizeLocation, UsizeValue,
};
use crate::mir::{DropPlan, DropPlanId, Place};

pub(super) fn lower_drop(
    context: &BackendContext<'_>,
    place: Place,
    plan: DropPlanId,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let ty = place
        .projection
        .map_or(context.body.locals[place.local.index()].ty, |projection| {
            context.body.projections[projection.index()].ty
        });
    let ty = context
        .typed_hir
        .type_expr_by_id(ty)
        .ok_or_else(|| invalid_mir_diagnostics("drop root type is missing"))?;
    if let Some((source, index)) = super::view_index_projection(place, context)? {
        let value = aggregate_abi_value(ty, context)?;
        let temporary = AggregateLocation::Slot(super::temporary_aggregate_slot(context));
        let mut instructions = vec![Instruction::CopySliceElementToAggregate {
            destination: temporary,
            source,
            index,
            layout: value.layout,
        }];
        instructions.extend(lower_plan(context, temporary, 0, ty, plan)?);
        return Ok(instructions);
    }
    let range = super::aggregate_range(place, 0, context)?;
    match range.index.as_ref() {
        Some(index) => lower_indexed_plan(context, range.location, range.offset, index, ty, plan),
        None => lower_plan(context, range.location, range.offset, ty, plan),
    }
}

fn lower_indexed_plan(
    context: &BackendContext<'_>,
    location: AggregateLocation,
    base_offset: u32,
    index: &AggregateIndex,
    ty: &crate::ast::TypeExpr,
    plan: DropPlanId,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let plan = context
        .body
        .drop_plans
        .get(plan.index())
        .ok_or_else(|| invalid_mir_diagnostics("indexed drop references a missing plan"))?;
    match plan {
        DropPlan::Noop => Ok(Vec::new()),
        DropPlan::Direct { destructor } => Ok(vec![direct_indexed_drop(
            context,
            location,
            base_offset,
            index,
            ty,
            *destructor,
        )?]),
        DropPlan::Struct { destructor, fields } => {
            let value = aggregate_abi_value(ty, context)?;
            let AbiType::Struct(abi_fields) = value.ty else {
                return Err(invalid_mir_diagnostics(
                    "indexed struct drop does not describe struct storage",
                ));
            };
            let layout = layout_struct(&abi_fields)
                .map_err(|error| invalid_mir_diagnostics(format!("{error:?}")))?;
            let mut instructions = Vec::new();
            if let Some(destructor) = destructor {
                instructions.push(direct_indexed_drop(
                    context,
                    location,
                    base_offset,
                    index,
                    ty,
                    *destructor,
                )?);
            }
            for field in fields.iter().rev() {
                let offset = layout
                    .fields
                    .get(field.index)
                    .and_then(|field| u32::try_from(field.offset).ok())
                    .and_then(|offset| base_offset.checked_add(offset))
                    .ok_or_else(|| {
                        invalid_mir_diagnostics("indexed drop field offset is invalid")
                    })?;
                instructions.extend(lower_indexed_plan(
                    context, location, offset, index, &field.ty, field.plan,
                )?);
            }
            Ok(instructions)
        }
        DropPlan::Array {
            length,
            element_ty,
            element,
        } => {
            let value = aggregate_abi_value(ty, context)?;
            let AbiType::Array { element: abi, .. } = value.ty else {
                return Err(invalid_mir_diagnostics(
                    "indexed array drop does not describe array storage",
                ));
            };
            let stride = crate::abi::array_element_stride(&abi)
                .map_err(|error| invalid_mir_diagnostics(format!("{error:?}")))?;
            let mut instructions = Vec::new();
            for child in (0..*length).rev() {
                let offset = child
                    .checked_mul(stride)
                    .and_then(|offset| u64::from(base_offset).checked_add(offset))
                    .and_then(|offset| u32::try_from(offset).ok())
                    .ok_or_else(|| {
                        invalid_mir_diagnostics("indexed nested array offset is invalid")
                    })?;
                instructions.extend(lower_indexed_plan(
                    context, location, offset, index, element_ty, *element,
                )?);
            }
            Ok(instructions)
        }
        DropPlan::Enum { variants } => {
            let value = aggregate_abi_value(ty, context)?;
            let AbiType::Enum(enum_) = value.ty else {
                return Err(invalid_mir_diagnostics(
                    "indexed enum drop does not describe enum storage",
                ));
            };
            let tag = U8Location::Local(super::storage::machine_local_count(context.body));
            let mut instructions = vec![Instruction::LoadAggregateU8Indexed {
                destination: tag,
                source: location,
                base_offset,
                index: index.value.clone(),
                length: index.length,
                stride: index.stride,
            }];
            for variant in variants.iter().rev() {
                let signature = super::enum_variant_signature(context, variant.definition)?;
                let abi_variant = enum_
                    .variants
                    .iter()
                    .find(|candidate| candidate.name == signature.name)
                    .ok_or_else(|| invalid_mir_diagnostics("indexed enum variant is missing"))?;
                let then_instructions = lower_indexed_enum_variant_fields(
                    context,
                    location,
                    base_offset,
                    index,
                    &enum_,
                    abi_variant,
                    variant,
                )?;
                instructions.push(Instruction::If {
                    condition: BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::Equal,
                        left: I32Value::U8ZeroExtend(Box::new(U8Value::Location(tag))),
                        right: I32Value::U8ZeroExtend(Box::new(U8Value::Const(abi_variant.tag))),
                    },
                    then_instructions,
                    else_instructions: Vec::new(),
                });
            }
            Ok(instructions)
        }
        DropPlan::Outcome {
            layers,
            payload_ty,
            payload,
        } => {
            let payload_value = aggregate_abi_value(payload_ty, context)?;
            let storage =
                crate::outcomes::storage::outcome_storage_layout(layers, payload_value.layout);
            let payload_offset = u32::try_from(storage.payload_offset)
                .ok()
                .and_then(|offset| base_offset.checked_add(offset))
                .ok_or_else(|| {
                    invalid_mir_diagnostics("indexed outcome payload offset is invalid")
                })?;
            let mut active = lower_indexed_plan(
                context,
                location,
                payload_offset,
                index,
                payload_ty,
                *payload,
            )?;
            let tag = UsizeLocation::Local(super::storage::machine_local_count(context.body));
            for layer in storage.layers.iter().rev() {
                let tag_offset = u32::try_from(layer.tag_offset)
                    .ok()
                    .and_then(|offset| base_offset.checked_add(offset))
                    .ok_or_else(|| {
                        invalid_mir_diagnostics("indexed outcome tag offset is invalid")
                    })?;
                active = vec![
                    Instruction::LoadAggregateUsizeIndexed {
                        destination: tag,
                        source: location,
                        base_offset: tag_offset,
                        index: index.value.clone(),
                        length: index.length,
                        stride: index.stride,
                    },
                    Instruction::If {
                        condition: BoolValue::UsizeComparison {
                            operator: I32ComparisonOperator::Equal,
                            left: UsizeValue::Location(tag),
                            right: UsizeValue::Const(0),
                        },
                        then_instructions: active,
                        else_instructions: Vec::new(),
                    },
                ];
            }
            Ok(active)
        }
    }
}

pub(super) fn lower_pointer_drop(
    context: &BackendContext<'_>,
    pointer: UsizeValue,
    offset: UsizeValue,
    ty: crate::semantic::TyId,
    plan: DropPlanId,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let ty = context
        .typed_hir
        .type_expr_by_id(ty)
        .ok_or_else(|| invalid_mir_diagnostics("pointer drop type is missing"))?;
    let value = aggregate_abi_value(ty, context)?;
    let temporary = AggregateLocation::Slot(super::temporary_aggregate_slot(context));
    let mut instructions = vec![Instruction::CopyPointerToAggregate {
        destination: temporary,
        pointer,
        offset,
        layout: value.layout,
    }];
    instructions.extend(lower_plan(context, temporary, 0, ty, plan)?);
    Ok(instructions)
}

fn lower_plan(
    context: &BackendContext<'_>,
    location: AggregateLocation,
    base_offset: u32,
    ty: &crate::ast::TypeExpr,
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
            ty,
            *destructor,
        )?]),
        DropPlan::Struct { destructor, fields } => {
            let value = aggregate_abi_value(ty, context)?;
            let AbiType::Struct(abi_fields) = value.ty else {
                return Err(invalid_mir_diagnostics(
                    "struct drop plan does not describe struct storage",
                ));
            };
            let layout = layout_struct(&abi_fields)
                .map_err(|error| invalid_mir_diagnostics(format!("{error:?}")))?;
            let mut instructions = Vec::new();
            if let Some(destructor) = destructor {
                instructions.push(direct_drop(
                    context,
                    location,
                    base_offset,
                    ty,
                    *destructor,
                )?);
            }
            for field in fields.iter().rev() {
                let offset = layout
                    .fields
                    .get(field.index)
                    .and_then(|field| u32::try_from(field.offset).ok())
                    .and_then(|offset| base_offset.checked_add(offset))
                    .ok_or_else(|| invalid_mir_diagnostics("drop field offset is invalid"))?;
                instructions.extend(lower_plan(
                    context, location, offset, &field.ty, field.plan,
                )?);
            }
            Ok(instructions)
        }
        DropPlan::Array {
            length,
            element_ty,
            element,
        } => {
            let value = aggregate_abi_value(ty, context)?;
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
                instructions.extend(lower_plan(context, location, offset, element_ty, *element)?);
            }
            Ok(instructions)
        }
        DropPlan::Enum { variants } => {
            let value = aggregate_abi_value(ty, context)?;
            let AbiType::Enum(enum_) = value.ty else {
                return Err(invalid_mir_diagnostics(
                    "enum drop plan does not describe enum storage",
                ));
            };
            let tag = U8Location::Local(super::storage::machine_local_count(context.body));
            let mut instructions = vec![Instruction::LoadAggregateU8 {
                destination: tag,
                source: location,
                offset: base_offset,
            }];
            for variant in variants.iter().rev() {
                let signature = super::enum_variant_signature(context, variant.definition)?;
                let abi_variant = enum_
                    .variants
                    .iter()
                    .find(|candidate| candidate.name == signature.name)
                    .ok_or_else(|| {
                        invalid_mir_diagnostics("enum drop variant has no matching ABI variant")
                    })?;
                let then_instructions = lower_enum_variant_fields(
                    context,
                    location,
                    base_offset,
                    &enum_,
                    abi_variant,
                    variant,
                )?;
                instructions.push(Instruction::If {
                    condition: crate::ir::BoolValue::I32Comparison {
                        operator: I32ComparisonOperator::Equal,
                        left: I32Value::U8ZeroExtend(Box::new(U8Value::Location(tag))),
                        right: I32Value::U8ZeroExtend(Box::new(U8Value::Const(abi_variant.tag))),
                    },
                    then_instructions,
                    else_instructions: Vec::new(),
                });
            }
            Ok(instructions)
        }
        DropPlan::Outcome {
            layers,
            payload_ty,
            payload,
        } => {
            let payload_value = aggregate_abi_value(payload_ty, context)?;
            let storage =
                crate::outcomes::storage::outcome_storage_layout(layers, payload_value.layout);
            let payload_offset = u32::try_from(storage.payload_offset)
                .ok()
                .and_then(|offset| base_offset.checked_add(offset))
                .ok_or_else(|| invalid_mir_diagnostics("outcome payload offset is invalid"))?;
            let mut active = lower_plan(context, location, payload_offset, payload_ty, *payload)?;
            for layer in storage.layers.iter().rev() {
                let tag_offset = u32::try_from(layer.tag_offset)
                    .ok()
                    .and_then(|offset| base_offset.checked_add(offset))
                    .ok_or_else(|| invalid_mir_diagnostics("outcome tag offset is invalid"))?;
                active = vec![Instruction::IfStoredOutcomeTag {
                    source: location,
                    tag_offset,
                    success_instructions: active,
                    outcome_instructions: Vec::new(),
                }];
            }
            Ok(active)
        }
    }
}

fn lower_enum_variant_fields(
    context: &BackendContext<'_>,
    location: AggregateLocation,
    base_offset: u32,
    enum_: &crate::abi::AbiEnum,
    abi_variant: &crate::abi::AbiEnumVariant,
    variant: &crate::mir::DropPlanVariant,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let payload_base = u32::try_from(enum_.payload_offset)
        .ok()
        .and_then(|offset| base_offset.checked_add(offset))
        .ok_or_else(|| invalid_mir_diagnostics("enum payload offset is invalid"))?;
    let field_offsets = match abi_variant.payload.as_ref() {
        Some(AbiType::Struct(fields)) if fields.len() > 1 => layout_struct(fields)
            .map_err(|error| invalid_mir_diagnostics(format!("{error:?}")))?
            .fields
            .into_iter()
            .map(|field| u32::try_from(field.offset).ok())
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| invalid_mir_diagnostics("enum payload field offset is invalid"))?,
        Some(_) => vec![0],
        None => Vec::new(),
    };
    let mut instructions = Vec::new();
    for field in variant.fields.iter().rev() {
        let offset = field_offsets
            .get(field.index)
            .and_then(|offset| payload_base.checked_add(*offset))
            .ok_or_else(|| invalid_mir_diagnostics("enum drop payload field is invalid"))?;
        instructions.extend(lower_plan(
            context, location, offset, &field.ty, field.plan,
        )?);
    }
    Ok(instructions)
}

fn lower_indexed_enum_variant_fields(
    context: &BackendContext<'_>,
    location: AggregateLocation,
    base_offset: u32,
    index: &AggregateIndex,
    enum_: &crate::abi::AbiEnum,
    abi_variant: &crate::abi::AbiEnumVariant,
    variant: &crate::mir::DropPlanVariant,
) -> Result<Vec<Instruction>, Vec<Diagnostic>> {
    let payload_base = u32::try_from(enum_.payload_offset)
        .ok()
        .and_then(|offset| base_offset.checked_add(offset))
        .ok_or_else(|| invalid_mir_diagnostics("indexed enum payload offset is invalid"))?;
    let field_offsets = match abi_variant.payload.as_ref() {
        Some(AbiType::Struct(fields)) if fields.len() > 1 => layout_struct(fields)
            .map_err(|error| invalid_mir_diagnostics(format!("{error:?}")))?
            .fields
            .into_iter()
            .map(|field| u32::try_from(field.offset).ok())
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| invalid_mir_diagnostics("indexed enum field offset is invalid"))?,
        Some(_) => vec![0],
        None => Vec::new(),
    };
    let mut instructions = Vec::new();
    for field in variant.fields.iter().rev() {
        let offset = field_offsets
            .get(field.index)
            .and_then(|offset| payload_base.checked_add(*offset))
            .ok_or_else(|| invalid_mir_diagnostics("indexed enum drop field is invalid"))?;
        instructions.extend(lower_indexed_plan(
            context, location, offset, index, &field.ty, field.plan,
        )?);
    }
    Ok(instructions)
}

fn direct_drop(
    context: &BackendContext<'_>,
    location: AggregateLocation,
    offset: u32,
    ty: &crate::ast::TypeExpr,
    destructor: crate::semantic::DefId,
) -> Result<Instruction, Vec<Diagnostic>> {
    let name = context
        .function_names
        .name_for_drop(destructor, ty)
        .ok_or_else(|| {
            invalid_mir_diagnostics(format!(
                "drop target {destructor:?} for `{}` has no indexed runtime name",
                crate::ast::canonical_type_expr(ty)
            ))
        })?
        .clone();
    let source = context
        .resolved
        .semantic_db
        .definition_anchor(destructor)
        .ok_or_else(|| invalid_mir_diagnostics("drop target has no source anchor"))?
        .source;
    let target = super::super::call_target_for_source(source, context.root_source, name);
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
        (AggregateLocation::Borrow(pointer), 0) => BorrowSource::BorrowLocal(pointer),
        (AggregateLocation::Borrow(pointer), offset) => {
            BorrowSource::BorrowLocalField { pointer, offset }
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

fn direct_indexed_drop(
    context: &BackendContext<'_>,
    location: AggregateLocation,
    base_offset: u32,
    index: &AggregateIndex,
    ty: &crate::ast::TypeExpr,
    destructor: crate::semantic::DefId,
) -> Result<Instruction, Vec<Diagnostic>> {
    let name = context
        .function_names
        .name_for_drop(destructor, ty)
        .ok_or_else(|| invalid_mir_diagnostics("indexed drop target has no runtime name"))?
        .clone();
    let source = context
        .resolved
        .semantic_db
        .definition_anchor(destructor)
        .ok_or_else(|| invalid_mir_diagnostics("indexed drop target has no source anchor"))?
        .source;
    let target = super::super::call_target_for_source(source, context.root_source, name);
    let index_value = match &index.value {
        UsizeValue::Const(value) => SliceElementIndex::Const(*value),
        UsizeValue::Location(location) => SliceElementIndex::Location(*location),
        _ => {
            return Err(invalid_mir_diagnostics(
                "indexed drop requires a direct usize index",
            ));
        }
    };
    Ok(Instruction::CallVoid {
        target,
        arguments: vec![ScalarArgument::Borrow(BorrowArgument {
            source: BorrowSource::AggregateIndex {
                source: location,
                base_offset,
                index: index_value,
                length: index.length,
                stride: index.stride,
            },
        })],
    })
}

fn aggregate_abi_value(
    ty: &crate::ast::TypeExpr,
    context: &BackendContext<'_>,
) -> Result<crate::abi::AbiValue, Vec<Diagnostic>> {
    crate::abi::abi_value_from_type_expr_with_resolver(ty, context.resolved, |source| {
        context.resolved_sources.get(&source).copied()
    })
    .map_err(|error| invalid_mir_diagnostics(format!("{error:?}")))
}
