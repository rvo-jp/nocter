use nocter_mir::{MirAggregate, MirClosureCapture};
use nocter_model::{FieldId, MirOperationId, MirValueId, TypeId, VariantId};

use super::body::BodyIdentities;
use super::{MachineAggregateError, MachineProgramError};
use crate::{
    MachineAggregate, MachineAggregateWrite, MachineLayoutKind, MachineLayoutPlan,
    MachineOutcomeKind,
};

pub(super) fn lower_aggregate(
    operation: MirOperationId,
    aggregate: &MirAggregate,
    ty: TypeId,
    layouts: &MachineLayoutPlan,
    ids: &BodyIdentities,
) -> Result<MachineAggregate, MachineProgramError> {
    let context = AggregateContext {
        operation,
        ids,
        layouts,
    };
    let layout = layouts
        .get(ty)
        .ok_or(MachineProgramError::MissingStoredLayout(ty))?;
    let writes = match (aggregate, layout.kind()) {
        (MirAggregate::Struct { fields, .. }, MachineLayoutKind::Struct { .. }) => {
            lower_struct(fields, context)?
        }
        (MirAggregate::Enum { variant, payload }, MachineLayoutKind::Enum { tag_offset, .. }) => {
            lower_enum(*variant, payload, *tag_offset, context)?
        }
        (
            MirAggregate::FixedArray(values),
            MachineLayoutKind::FixedArray { length, stride, .. },
        ) => lower_fixed_array(values, *length, *stride, context)?,
        (
            MirAggregate::Optional(payload),
            MachineLayoutKind::Outcome {
                kind: MachineOutcomeKind::Optional,
                tag_offset,
                payload_offset,
                ..
            },
        ) => outcome_writes(
            *tag_offset,
            *payload_offset,
            payload.as_ref().copied(),
            MachineOutcomeKind::Optional.primary_tag(),
            MachineOutcomeKind::Optional.alternate_tag(),
            context,
        )?,
        (
            MirAggregate::FallibleSuccess(payload),
            MachineLayoutKind::Outcome {
                kind: MachineOutcomeKind::Fallible,
                tag_offset,
                payload_offset,
                ..
            },
        ) => outcome_writes(
            *tag_offset,
            *payload_offset,
            payload.as_ref().copied(),
            MachineOutcomeKind::Fallible.primary_tag(),
            MachineOutcomeKind::Fallible.primary_tag(),
            context,
        )?,
        (
            MirAggregate::FallibleFailure(error),
            MachineLayoutKind::Outcome {
                kind: MachineOutcomeKind::Fallible,
                tag_offset,
                payload_offset,
                ..
            },
        ) => outcome_writes(
            *tag_offset,
            *payload_offset,
            Some(*error),
            MachineOutcomeKind::Fallible.alternate_tag(),
            MachineOutcomeKind::Fallible.alternate_tag(),
            context,
        )?,
        (MirAggregate::Closure { captures, .. }, MachineLayoutKind::Closure { .. }) => {
            lower_closure(captures, context)?
        }
        (MirAggregate::Opaque { witness }, MachineLayoutKind::Opaque { .. }) => {
            vec![MachineAggregateWrite::Value {
                offset: 0,
                value: context.ids.value(*witness)?,
            }]
        }
        _ => return Err(context.error(MachineAggregateError::InvalidLayout)),
    };
    Ok(MachineAggregate::new(
        layout.size(),
        layout.alignment(),
        writes,
    ))
}

#[derive(Clone, Copy)]
struct AggregateContext<'a> {
    operation: MirOperationId,
    ids: &'a BodyIdentities,
    layouts: &'a MachineLayoutPlan,
}

impl AggregateContext<'_> {
    const fn error(self, error: MachineAggregateError) -> MachineProgramError {
        MachineProgramError::Aggregate {
            owner: self.ids.owner(),
            operation: self.operation,
            error,
        }
    }
}

fn lower_struct(
    fields: &[(FieldId, MirValueId)],
    context: AggregateContext<'_>,
) -> Result<Vec<MachineAggregateWrite>, MachineProgramError> {
    fields
        .iter()
        .map(|(field, value)| {
            let offset = context
                .layouts
                .field(*field)
                .map(crate::MachineFieldLayout::offset)
                .ok_or_else(|| context.error(MachineAggregateError::MemberMismatch))?;
            Ok(MachineAggregateWrite::Value {
                offset,
                value: context.ids.value(*value)?,
            })
        })
        .collect()
}

fn lower_enum(
    variant: VariantId,
    payload: &[MirValueId],
    tag_offset: u64,
    context: AggregateContext<'_>,
) -> Result<Vec<MachineAggregateWrite>, MachineProgramError> {
    let variant = context
        .layouts
        .variant(variant)
        .ok_or_else(|| context.error(MachineAggregateError::MemberMismatch))?;
    if variant.payload().len() != payload.len() {
        return Err(context.error(MachineAggregateError::MemberMismatch));
    }
    let mut writes = Vec::with_capacity(payload.len() + 1);
    writes.push(MachineAggregateWrite::Tag {
        offset: tag_offset,
        value: variant.tag(),
    });
    for (value, member) in payload.iter().zip(variant.payload()) {
        writes.push(MachineAggregateWrite::Value {
            offset: member.offset(),
            value: context.ids.value(*value)?,
        });
    }
    Ok(writes)
}

fn lower_fixed_array(
    values: &[MirValueId],
    length: u64,
    stride: u64,
    context: AggregateContext<'_>,
) -> Result<Vec<MachineAggregateWrite>, MachineProgramError> {
    if usize::try_from(length) != Ok(values.len()) {
        return Err(context.error(MachineAggregateError::MemberMismatch));
    }
    values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let offset = u64::try_from(index)
                .ok()
                .and_then(|index| index.checked_mul(stride))
                .ok_or_else(|| context.error(MachineAggregateError::OffsetOverflow))?;
            Ok(MachineAggregateWrite::Value {
                offset,
                value: context.ids.value(*value)?,
            })
        })
        .collect()
}

fn lower_closure(
    captures: &[MirClosureCapture],
    context: AggregateContext<'_>,
) -> Result<Vec<MachineAggregateWrite>, MachineProgramError> {
    captures
        .iter()
        .map(|capture| {
            let offset = context
                .layouts
                .capture(capture.binding())
                .map(crate::MachineCaptureLayout::offset)
                .ok_or_else(|| context.error(MachineAggregateError::MemberMismatch))?;
            Ok(MachineAggregateWrite::Value {
                offset,
                value: context.ids.value(capture.value())?,
            })
        })
        .collect()
}

fn outcome_writes(
    tag_offset: u64,
    payload_offset: u64,
    payload: Option<MirValueId>,
    present_tag: u8,
    empty_tag: u8,
    context: AggregateContext<'_>,
) -> Result<Vec<MachineAggregateWrite>, MachineProgramError> {
    let mut writes = Vec::with_capacity(usize::from(payload.is_some()) + 1);
    writes.push(MachineAggregateWrite::Tag {
        offset: tag_offset,
        value: if payload.is_some() {
            present_tag
        } else {
            empty_tag
        },
    });
    if let Some(payload) = payload {
        writes.push(MachineAggregateWrite::Value {
            offset: payload_offset,
            value: context.ids.value(payload)?,
        });
    }
    Ok(writes)
}
