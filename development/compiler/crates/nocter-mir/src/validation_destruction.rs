use std::collections::BTreeSet;

use nocter_model::{BuiltinType, TypeKind, TypeStore};
use nocter_runtime_contract::RuntimeTypeRepresentation;

use crate::validation_types::matches_opaque_projection;
use crate::{MirDestructionKind, MirDestructionPlan, MirValidationEnvironment, MirValidationError};

pub(crate) fn validate_destruction_plan(
    environment: &(impl MirValidationEnvironment + ?Sized),
    plan: &MirDestructionPlan,
) -> Result<(), MirValidationError> {
    let types = environment.types();
    if types.get(plan.ty()).is_none() {
        return Err(MirValidationError::UnknownType(plan.ty()));
    }
    match plan.kind() {
        MirDestructionKind::Struct { drop, fields } => {
            require_drop_items(environment, drop.iter().copied())?;
            validate_struct(environment, types, plan, fields)?;
        }
        MirDestructionKind::Enum { drop, variants } => {
            require_drop_items(environment, drop.iter().copied())?;
            validate_enum(environment, types, plan, variants)?;
        }
        MirDestructionKind::FixedArray { length, element } => {
            if !matches!(
                types.get(plan.ty()),
                Some(TypeKind::FixedArray {
                    element: actual,
                    length: actual_length,
                }) if actual == &element.ty() && actual_length == length
            ) {
                return Err(MirValidationError::InvalidDestruction(plan.ty()));
            }
            validate_destruction_plan(environment, element)?;
        }
        MirDestructionKind::Optional(payload) => {
            if !matches!(types.get(plan.ty()), Some(TypeKind::Optional(actual)) if actual == &payload.ty())
            {
                return Err(MirValidationError::InvalidDestruction(plan.ty()));
            }
            validate_destruction_plan(environment, payload)?;
        }
        MirDestructionKind::Fallible { success, failure } => {
            if !matches!(types.get(plan.ty()), Some(TypeKind::Fallible(actual)) if success.as_deref().is_none_or(|payload| actual == &payload.ty()))
                || types.get(failure.ty()) != Some(&TypeKind::Builtin(BuiltinType::Error))
            {
                return Err(MirValidationError::InvalidDestruction(plan.ty()));
            }
            if let Some(success) = success {
                validate_destruction_plan(environment, success)?;
            }
            validate_destruction_plan(environment, failure)?;
        }
        MirDestructionKind::Error => {
            if types.get(plan.ty()) != Some(&TypeKind::Builtin(BuiltinType::Error)) {
                return Err(MirValidationError::InvalidDestruction(plan.ty()));
            }
        }
        MirDestructionKind::Closure(captures) => {
            validate_closure(environment, types, plan, captures)?;
        }
        MirDestructionKind::Opaque {
            definition,
            plan: inner,
        } => {
            if !matches_opaque_projection(environment, types, plan.ty(), *definition, inner.ty()) {
                return Err(MirValidationError::InvalidDestruction(plan.ty()));
            }
            validate_destruction_plan(environment, inner)?;
        }
    }
    Ok(())
}

fn validate_struct(
    environment: &(impl MirValidationEnvironment + ?Sized),
    types: &TypeStore,
    plan: &MirDestructionPlan,
    fields: &[crate::MirFieldDestruction],
) -> Result<(), MirValidationError> {
    if !matches!(types.get(plan.ty()), Some(TypeKind::Nominal { .. })) {
        return Err(MirValidationError::InvalidDestruction(plan.ty()));
    }
    let Some(RuntimeTypeRepresentation::Struct { fields: declared }) =
        environment.type_representation(plan.ty())
    else {
        return Err(MirValidationError::InvalidDestruction(plan.ty()));
    };
    let mut seen = BTreeSet::new();
    let mut previous = declared.len();
    for field in fields {
        let Some(position) = declared
            .iter()
            .position(|candidate| candidate.field() == field.field())
            .filter(|position| *position < previous)
        else {
            return Err(MirValidationError::InvalidDestruction(plan.ty()));
        };
        if !seen.insert(field.field()) || declared[position].ty() != field.plan().ty() {
            return Err(MirValidationError::InvalidDestruction(plan.ty()));
        }
        previous = position;
        validate_destruction_plan(environment, field.plan())?;
    }
    Ok(())
}

fn validate_closure(
    environment: &(impl MirValidationEnvironment + ?Sized),
    types: &TypeStore,
    plan: &MirDestructionPlan,
    captures: &[crate::MirCaptureDestruction],
) -> Result<(), MirValidationError> {
    if !matches!(types.get(plan.ty()), Some(TypeKind::Closure { .. })) {
        return Err(MirValidationError::InvalidDestruction(plan.ty()));
    }
    let Some(RuntimeTypeRepresentation::Closure { captures: declared }) =
        environment.type_representation(plan.ty())
    else {
        return Err(MirValidationError::InvalidDestruction(plan.ty()));
    };
    let mut seen = BTreeSet::new();
    let mut previous = declared.len();
    for capture in captures {
        let Some(position) = declared
            .iter()
            .position(|candidate| candidate.capture() == capture.capture())
            .filter(|position| *position < previous)
        else {
            return Err(MirValidationError::InvalidDestruction(plan.ty()));
        };
        if !seen.insert(capture.capture()) || declared[position].ty() != capture.plan().ty() {
            return Err(MirValidationError::InvalidDestruction(plan.ty()));
        }
        previous = position;
        validate_destruction_plan(environment, capture.plan())?;
    }
    Ok(())
}

fn validate_enum(
    environment: &(impl MirValidationEnvironment + ?Sized),
    types: &TypeStore,
    plan: &MirDestructionPlan,
    variants: &[crate::MirVariantDestruction],
) -> Result<(), MirValidationError> {
    if !matches!(types.get(plan.ty()), Some(TypeKind::Nominal { .. })) {
        return Err(MirValidationError::InvalidDestruction(plan.ty()));
    }
    let Some(RuntimeTypeRepresentation::Enum { variants: declared }) =
        environment.type_representation(plan.ty())
    else {
        return Err(MirValidationError::InvalidDestruction(plan.ty()));
    };
    let mut seen_variants = BTreeSet::new();
    let mut previous_variant = None;
    for variant in variants {
        let position = declared
            .iter()
            .position(|candidate| candidate.variant() == variant.variant())
            .filter(|position| previous_variant.is_none_or(|old| old < *position));
        let Some(position) = position else {
            return Err(MirValidationError::InvalidDestruction(plan.ty()));
        };
        if !seen_variants.insert(variant.variant()) {
            return Err(MirValidationError::InvalidDestruction(plan.ty()));
        }
        previous_variant = Some(position);
        let declaration = &declared[position];
        let mut seen_payload = BTreeSet::new();
        let mut previous_payload = declaration.payload().len();
        for payload in variant.payload() {
            let Some(position) = declaration
                .payload()
                .iter()
                .position(|candidate| candidate.parameter() == payload.parameter())
                .filter(|position| *position < previous_payload)
            else {
                return Err(MirValidationError::InvalidDestruction(plan.ty()));
            };
            if !seen_payload.insert(payload.parameter())
                || declaration.payload()[position].ty() != payload.plan().ty()
            {
                return Err(MirValidationError::InvalidDestruction(plan.ty()));
            }
            previous_payload = position;
            validate_destruction_plan(environment, payload.plan())?;
        }
    }
    Ok(())
}

fn require_drop_items(
    environment: &(impl MirValidationEnvironment + ?Sized),
    items: impl IntoIterator<Item = nocter_model::ExecutableItemId>,
) -> Result<(), MirValidationError> {
    for item in items {
        if !environment.contains_item(item) {
            return Err(MirValidationError::UnknownItem(item));
        }
    }
    Ok(())
}
