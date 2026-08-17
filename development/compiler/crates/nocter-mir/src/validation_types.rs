use std::collections::BTreeMap;

use nocter_model::{
    BuiltinType, GenericParameterId, MirPlaceId, NominalTypeId, TypeId, TypeKind, TypeStore,
};

use crate::{MirValidationEnvironment, MirValidationError};

pub(crate) fn nominal_application(
    types: &TypeStore,
    ty: TypeId,
    place: MirPlaceId,
) -> Result<(NominalTypeId, &[TypeId]), MirValidationError> {
    match types.get(ty) {
        Some(TypeKind::Nominal {
            definition,
            arguments,
        }) => Ok((*definition, arguments)),
        _ => Err(MirValidationError::InvalidProjection { place }),
    }
}

pub(crate) fn is_integer(types: &TypeStore, ty: TypeId) -> bool {
    matches!(
        types.get(ty),
        Some(TypeKind::Builtin(
            BuiltinType::I8
                | BuiltinType::I16
                | BuiltinType::I32
                | BuiltinType::I64
                | BuiltinType::U8
                | BuiltinType::U16
                | BuiltinType::U32
                | BuiltinType::U64
                | BuiltinType::Usize
                | BuiltinType::Isize
        ))
    )
}

pub(crate) fn matches_nominal_member(
    environment: &(impl MirValidationEnvironment + ?Sized),
    types: &TypeStore,
    definition: NominalTypeId,
    arguments: &[TypeId],
    pattern: TypeId,
    actual: TypeId,
) -> bool {
    let Some(declaration) = environment.nominal_type(definition) else {
        return false;
    };
    if declaration.generic_parameters().len() != arguments.len() {
        return false;
    }
    let substitution = declaration
        .generic_parameters()
        .iter()
        .copied()
        .zip(arguments.iter().copied())
        .collect::<BTreeMap<_, _>>();
    matches_type(types, pattern, actual, &substitution)
}

fn matches_type(
    types: &TypeStore,
    pattern: TypeId,
    actual: TypeId,
    substitution: &BTreeMap<GenericParameterId, TypeId>,
) -> bool {
    if let Some(TypeKind::GenericParameter(parameter)) = types.get(pattern) {
        return substitution.get(parameter) == Some(&actual);
    }
    match (types.get(pattern), types.get(actual)) {
        (Some(TypeKind::Builtin(left)), Some(TypeKind::Builtin(right))) => left == right,
        (
            Some(TypeKind::Nominal {
                definition: left,
                arguments: left_arguments,
            }),
            Some(TypeKind::Nominal {
                definition: right,
                arguments: right_arguments,
            }),
        ) => same_application(
            types,
            left,
            right,
            left_arguments,
            right_arguments,
            substitution,
        ),
        (
            Some(TypeKind::Opaque {
                definition: left,
                arguments: left_arguments,
            }),
            Some(TypeKind::Opaque {
                definition: right,
                arguments: right_arguments,
            }),
        ) => same_application(
            types,
            left,
            right,
            left_arguments,
            right_arguments,
            substitution,
        ),
        (Some(TypeKind::Pointer(left)), Some(TypeKind::Pointer(right)))
        | (Some(TypeKind::Slice(left)), Some(TypeKind::Slice(right)))
        | (Some(TypeKind::Optional(left)), Some(TypeKind::Optional(right)))
        | (Some(TypeKind::Fallible(left)), Some(TypeKind::Fallible(right))) => {
            matches_type(types, *left, *right, substitution)
        }
        (
            Some(TypeKind::Borrow {
                capability: left_capability,
                referent: left,
            }),
            Some(TypeKind::Borrow {
                capability: right_capability,
                referent: right,
            }),
        ) => {
            left_capability == right_capability && matches_type(types, *left, *right, substitution)
        }
        (
            Some(TypeKind::FixedArray {
                element: left,
                length: left_length,
            }),
            Some(TypeKind::FixedArray {
                element: right,
                length: right_length,
            }),
        ) => left_length == right_length && matches_type(types, *left, *right, substitution),
        _ => pattern == actual,
    }
}

fn same_application<I: Eq>(
    types: &TypeStore,
    left: &I,
    right: &I,
    left_arguments: &[TypeId],
    right_arguments: &[TypeId],
    substitution: &BTreeMap<GenericParameterId, TypeId>,
) -> bool {
    left == right
        && left_arguments.len() == right_arguments.len()
        && left_arguments
            .iter()
            .copied()
            .zip(right_arguments.iter().copied())
            .all(|(left, right)| matches_type(types, left, right, substitution))
}
