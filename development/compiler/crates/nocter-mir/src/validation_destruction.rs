use std::collections::BTreeSet;

use nocter_declarations::{NominalShape, ParameterOwner};
use nocter_model::{TypeKind, TypeStore};

use crate::validation_types::{matches_nominal_member, matches_opaque_projection};
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
        MirDestructionKind::Fallible(payload) => {
            if !matches!(types.get(plan.ty()), Some(TypeKind::Fallible(actual)) if actual == &payload.ty())
            {
                return Err(MirValidationError::InvalidDestruction(plan.ty()));
            }
            validate_destruction_plan(environment, payload)?;
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
    let Some(TypeKind::Nominal {
        definition,
        arguments,
    }) = types.get(plan.ty())
    else {
        return Err(MirValidationError::InvalidDestruction(plan.ty()));
    };
    let Some(NominalShape::Struct {
        fields: declared, ..
    }) = environment
        .nominal_type(*definition)
        .map(nocter_declarations::NominalTypeDeclaration::shape)
    else {
        return Err(MirValidationError::InvalidDestruction(plan.ty()));
    };
    let mut seen = BTreeSet::new();
    let mut previous = declared.len();
    for field in fields {
        let declaration = environment
            .field(field.field())
            .ok_or(MirValidationError::UnknownField(field.field()))?;
        let Some(position) = descending_position(&field.field(), declared, previous) else {
            return Err(MirValidationError::InvalidDestruction(plan.ty()));
        };
        if !seen.insert(field.field())
            || declaration.owner() != *definition
            || !matches_nominal_member(
                environment,
                types,
                *definition,
                arguments,
                declaration.ty(),
                field.plan().ty(),
            )
        {
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
    let layout = environment
        .closure_layout_for_type(plan.ty())
        .ok_or(MirValidationError::InvalidDestruction(plan.ty()))?;
    let bindings = layout
        .captures()
        .iter()
        .map(|capture| capture.binding())
        .collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    let mut previous = bindings.len();
    for capture in captures {
        let Some(position) = descending_position(&capture.capture(), &bindings, previous) else {
            return Err(MirValidationError::InvalidDestruction(plan.ty()));
        };
        if !seen.insert(capture.capture())
            || environment.closure_capture_type(plan.ty(), capture.capture())
                != Some(capture.plan().ty())
        {
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
    let Some(TypeKind::Nominal {
        definition,
        arguments,
    }) = types.get(plan.ty())
    else {
        return Err(MirValidationError::InvalidDestruction(plan.ty()));
    };
    let Some(NominalShape::Enum { variants: declared }) = environment
        .nominal_type(*definition)
        .map(nocter_declarations::NominalTypeDeclaration::shape)
    else {
        return Err(MirValidationError::InvalidDestruction(plan.ty()));
    };
    let mut seen_variants = BTreeSet::new();
    let mut previous_variant = None;
    for variant in variants {
        let declaration = environment
            .variant(variant.variant())
            .ok_or(MirValidationError::UnknownVariant(variant.variant()))?;
        let position = declared
            .iter()
            .position(|candidate| candidate == &variant.variant())
            .filter(|position| previous_variant.is_none_or(|old| old < *position));
        let Some(position) = position else {
            return Err(MirValidationError::InvalidDestruction(plan.ty()));
        };
        if !seen_variants.insert(variant.variant()) || declaration.owner() != *definition {
            return Err(MirValidationError::InvalidDestruction(plan.ty()));
        }
        previous_variant = Some(position);
        let mut seen_payload = BTreeSet::new();
        let mut previous_payload = declaration.payload().len();
        for payload in variant.payload() {
            let parameter = environment
                .parameter(payload.parameter())
                .ok_or(MirValidationError::UnknownParameter(payload.parameter()))?;
            let Some(position) = descending_position(
                &payload.parameter(),
                declaration.payload(),
                previous_payload,
            ) else {
                return Err(MirValidationError::InvalidDestruction(plan.ty()));
            };
            if !seen_payload.insert(payload.parameter())
                || parameter.owner() != ParameterOwner::Variant(variant.variant())
                || !matches_nominal_member(
                    environment,
                    types,
                    *definition,
                    arguments,
                    parameter.ty(),
                    payload.plan().ty(),
                )
            {
                return Err(MirValidationError::InvalidDestruction(plan.ty()));
            }
            previous_payload = position;
            validate_destruction_plan(environment, payload.plan())?;
        }
    }
    Ok(())
}

fn descending_position<T: Eq>(value: &T, declared: &[T], before: usize) -> Option<usize> {
    declared
        .iter()
        .position(|candidate| candidate == value)
        .filter(|position| *position < before)
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
