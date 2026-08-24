use std::collections::BTreeMap;

use nocter_mir::{MirDestructionKind, MirDestructionPlan};
use nocter_model::{ExecutableItemId, MirOperationId};

use super::MachineProgramError;
use crate::{
    MachineDestructionCapture, MachineDestructionError, MachineDestructionField,
    MachineDestructionKind, MachineDestructionPayload, MachineDestructionPlan,
    MachineDestructionVariant, MachineFunctionId, MachineLayoutKind, MachineLayoutStore,
    MachineLinkageId, MachineOutcomeKind,
};

#[derive(Clone, Copy)]
struct DestructionContext<'a> {
    owner: MachineLinkageId,
    operation: MirOperationId,
    layouts: &'a MachineLayoutStore,
    functions: &'a BTreeMap<ExecutableItemId, MachineFunctionId>,
}

impl DestructionContext<'_> {
    const fn error(self, error: MachineDestructionError) -> MachineProgramError {
        MachineProgramError::Destruction {
            owner: self.owner,
            operation: self.operation,
            error,
        }
    }
}

pub(crate) fn lower_destruction(
    plan: &MirDestructionPlan,
    owner: MachineLinkageId,
    operation: MirOperationId,
    layouts: &MachineLayoutStore,
    functions: &BTreeMap<ExecutableItemId, MachineFunctionId>,
) -> Result<MachineDestructionPlan, MachineProgramError> {
    lower_plan(
        plan,
        DestructionContext {
            owner,
            operation,
            layouts,
            functions,
        },
    )
}

fn lower_plan(
    plan: &MirDestructionPlan,
    context: DestructionContext<'_>,
) -> Result<MachineDestructionPlan, MachineProgramError> {
    let layout = context
        .layouts
        .get(plan.ty())
        .ok_or_else(|| context.error(MachineDestructionError::InvalidLayout(plan.ty())))?;
    let kind = lower_kind(plan, layout.kind(), context)?;
    Ok(MachineDestructionPlan::new(
        plan.ty(),
        layout.size(),
        layout.alignment(),
        kind,
    ))
}

fn lower_kind(
    plan: &MirDestructionPlan,
    layout: &MachineLayoutKind,
    context: DestructionContext<'_>,
) -> Result<MachineDestructionKind, MachineProgramError> {
    match (plan.kind(), layout) {
        (
            MirDestructionKind::Struct { drop, fields },
            MachineLayoutKind::Struct { fields: members },
        ) => lower_struct(plan.ty(), *drop, fields, members, context),
        (
            MirDestructionKind::Enum { drop, variants },
            MachineLayoutKind::Enum {
                tag_offset,
                variants: members,
                ..
            },
        ) => lower_enum(plan.ty(), *drop, *tag_offset, variants, members, context),
        (
            MirDestructionKind::FixedArray { length, element },
            MachineLayoutKind::FixedArray {
                element: actual,
                length: actual_length,
                stride,
            },
        ) if element.ty() == *actual && *length == *actual_length => {
            Ok(MachineDestructionKind::FixedArray {
                length: *length,
                stride: *stride,
                element: Box::new(lower_plan(element, context)?),
            })
        }
        (
            MirDestructionKind::Optional(payload),
            MachineLayoutKind::Outcome {
                kind: MachineOutcomeKind::Optional,
                tag_offset,
                payload_offset,
                primary: Some(primary),
                ..
            },
        ) if payload.ty() == *primary => Ok(MachineDestructionKind::Outcome {
            tag_offset: *tag_offset,
            payload_offset: *payload_offset,
            active_tag: MachineOutcomeKind::Optional.primary_tag(),
            payload: Box::new(lower_plan(payload, context)?),
        }),
        (
            MirDestructionKind::Fallible { success, failure },
            MachineLayoutKind::Outcome {
                kind: MachineOutcomeKind::Fallible,
                tag_offset,
                payload_offset,
                primary,
                ..
            },
        ) if match (success.as_deref(), primary) {
            (Some(payload), Some(primary)) => payload.ty() == *primary,
            (None, None) => true,
            _ => false,
        } =>
        {
            Ok(MachineDestructionKind::Fallible {
                tag_offset: *tag_offset,
                payload_offset: *payload_offset,
                success: success
                    .as_deref()
                    .map(|plan| lower_plan(plan, context).map(Box::new))
                    .transpose()?,
                failure: Box::new(lower_plan(failure, context)?),
            })
        }
        (MirDestructionKind::Error, MachineLayoutKind::ErrorHandle) => {
            Ok(MachineDestructionKind::Error)
        }
        (
            MirDestructionKind::Closure(captures),
            MachineLayoutKind::Closure { captures: members },
        ) => lower_closure(plan.ty(), captures, members, context),
        (MirDestructionKind::Opaque { plan: inner, .. }, MachineLayoutKind::Opaque { witness })
            if inner.ty() == *witness =>
        {
            Ok(MachineDestructionKind::Opaque(Box::new(lower_plan(
                inner, context,
            )?)))
        }
        _ => Err(context.error(MachineDestructionError::InvalidLayout(plan.ty()))),
    }
}

fn lower_struct(
    ty: nocter_model::TypeId,
    drop: Option<ExecutableItemId>,
    fields: &[nocter_mir::MirFieldDestruction],
    members: &[crate::MachineFieldLayout],
    context: DestructionContext<'_>,
) -> Result<MachineDestructionKind, MachineProgramError> {
    let fields = fields
        .iter()
        .map(|field| {
            let offset = members
                .iter()
                .find(|member| member.field() == field.field())
                .map(|member| member.offset())
                .ok_or_else(|| context.error(MachineDestructionError::MissingMember(ty)))?;
            Ok(MachineDestructionField::new(
                offset,
                lower_plan(field.plan(), context)?,
            ))
        })
        .collect::<Result<Vec<_>, MachineProgramError>>()?;
    Ok(MachineDestructionKind::Struct {
        drop: lower_drop(drop, context)?,
        fields: fields.into_boxed_slice(),
    })
}

fn lower_enum(
    ty: nocter_model::TypeId,
    drop: Option<ExecutableItemId>,
    tag_offset: u64,
    variants: &[nocter_mir::MirVariantDestruction],
    members: &[crate::MachineEnumVariantLayout],
    context: DestructionContext<'_>,
) -> Result<MachineDestructionKind, MachineProgramError> {
    let variants = variants
        .iter()
        .map(|variant| {
            let member = members
                .iter()
                .find(|member| member.variant() == variant.variant())
                .ok_or_else(|| context.error(MachineDestructionError::MissingMember(ty)))?;
            let payload = variant
                .payload()
                .iter()
                .map(|payload| {
                    let offset = member
                        .payload()
                        .iter()
                        .find(|candidate| candidate.parameter() == payload.parameter())
                        .map(|candidate| candidate.offset())
                        .ok_or_else(|| context.error(MachineDestructionError::MissingMember(ty)))?;
                    Ok(MachineDestructionPayload::new(
                        offset,
                        lower_plan(payload.plan(), context)?,
                    ))
                })
                .collect::<Result<Vec<_>, MachineProgramError>>()?;
            Ok(MachineDestructionVariant::new(member.tag(), payload))
        })
        .collect::<Result<Vec<_>, MachineProgramError>>()?;
    Ok(MachineDestructionKind::Enum {
        drop: lower_drop(drop, context)?,
        tag_offset,
        variants: variants.into_boxed_slice(),
    })
}

fn lower_closure(
    ty: nocter_model::TypeId,
    captures: &[nocter_mir::MirCaptureDestruction],
    members: &[crate::MachineCaptureLayout],
    context: DestructionContext<'_>,
) -> Result<MachineDestructionKind, MachineProgramError> {
    captures
        .iter()
        .map(|capture| {
            let offset = members
                .iter()
                .find(|member| member.capture() == capture.capture())
                .map(|member| member.offset())
                .ok_or_else(|| context.error(MachineDestructionError::MissingMember(ty)))?;
            Ok(MachineDestructionCapture::new(
                offset,
                lower_plan(capture.plan(), context)?,
            ))
        })
        .collect::<Result<Vec<_>, MachineProgramError>>()
        .map(|captures| MachineDestructionKind::Closure(captures.into_boxed_slice()))
}

fn lower_drop(
    drop: Option<ExecutableItemId>,
    context: DestructionContext<'_>,
) -> Result<Option<MachineFunctionId>, MachineProgramError> {
    drop.map(|drop| {
        context
            .functions
            .get(&drop)
            .copied()
            .ok_or(MachineProgramError::MissingItemFunction(drop))
    })
    .transpose()
}
