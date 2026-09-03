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
        MirDestructionKind::Tuple(elements) => {
            let Some(TypeKind::Tuple(declared)) = types.get(plan.ty()) else {
                return Err(MirValidationError::InvalidDestruction(plan.ty()));
            };
            let mut previous = None;
            for element in elements {
                if previous.is_some_and(|previous| previous <= element.index())
                    || declared.get(element.index()) != Some(element.plan().ty())
                {
                    return Err(MirValidationError::InvalidDestruction(plan.ty()));
                }
                previous = Some(element.index());
                validate_destruction_plan(environment, element.plan())?;
            }
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
    let mut previous = declared.len();
    for field in fields {
        let Some(position) = declared[..previous]
            .iter()
            .rposition(|candidate| candidate.field() == field.field())
        else {
            return Err(MirValidationError::InvalidDestruction(plan.ty()));
        };
        if declared[position].ty() != field.plan().ty() {
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
    let mut previous = declared.len();
    for capture in captures {
        let Some(position) = declared[..previous]
            .iter()
            .rposition(|candidate| candidate.capture() == capture.capture())
        else {
            return Err(MirValidationError::InvalidDestruction(plan.ty()));
        };
        if declared[position].ty() != capture.plan().ty() {
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
    let mut previous_variant = None;
    for variant in variants {
        let start = previous_variant.map_or(0, |previous| previous + 1);
        let position = declared[start..]
            .iter()
            .position(|candidate| candidate.variant() == variant.variant())
            .map(|relative| start + relative);
        let Some(position) = position else {
            return Err(MirValidationError::InvalidDestruction(plan.ty()));
        };
        previous_variant = Some(position);
        let declaration = &declared[position];
        let mut previous_payload = declaration.payload().len();
        for payload in variant.payload() {
            let Some(position) = declaration.payload()[..previous_payload]
                .iter()
                .rposition(|candidate| candidate.parameter() == payload.parameter())
            else {
                return Err(MirValidationError::InvalidDestruction(plan.ty()));
            };
            if declaration.payload()[position].ty() != payload.plan().ty() {
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
