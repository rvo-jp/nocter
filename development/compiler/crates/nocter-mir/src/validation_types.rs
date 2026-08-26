use nocter_model::{
    BuiltinType, CaptureId, FieldId, OpaqueTypeId, ParameterId, TypeId, TypeKind, TypeStore,
    VariantId,
};
use nocter_runtime_contract::{RuntimeTypeRepresentation, RuntimeVariantRepresentation};

use crate::MirValidationEnvironment;

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

pub(crate) fn field_type(
    environment: &(impl MirValidationEnvironment + ?Sized),
    owner: TypeId,
    field: FieldId,
) -> Option<TypeId> {
    let RuntimeTypeRepresentation::Struct { fields } = environment.type_representation(owner)?
    else {
        return None;
    };
    fields
        .iter()
        .copied()
        .find(|candidate| candidate.field() == field)
        .map(nocter_runtime_contract::RuntimeFieldRepresentation::ty)
}

pub(crate) fn capture_type(
    environment: &(impl MirValidationEnvironment + ?Sized),
    owner: TypeId,
    capture: CaptureId,
) -> Option<TypeId> {
    let RuntimeTypeRepresentation::Closure { captures } = environment.type_representation(owner)?
    else {
        return None;
    };
    captures
        .iter()
        .copied()
        .find(|candidate| candidate.capture() == capture)
        .map(nocter_runtime_contract::RuntimeCaptureRepresentation::ty)
}

pub(crate) fn variant_representation(
    environment: &(impl MirValidationEnvironment + ?Sized),
    owner: TypeId,
    variant: VariantId,
) -> Option<&RuntimeVariantRepresentation> {
    let RuntimeTypeRepresentation::Enum { variants } = environment.type_representation(owner)?
    else {
        return None;
    };
    variants
        .iter()
        .find(|candidate| candidate.variant() == variant)
}

pub(crate) fn payload_type(
    environment: &(impl MirValidationEnvironment + ?Sized),
    owner: TypeId,
    variant: VariantId,
    parameter: ParameterId,
) -> Option<TypeId> {
    variant_representation(environment, owner, variant)?
        .payload()
        .iter()
        .copied()
        .find(|candidate| candidate.parameter() == parameter)
        .map(nocter_runtime_contract::RuntimePayloadRepresentation::ty)
}

pub(crate) fn matches_opaque_witness(
    environment: &(impl MirValidationEnvironment + ?Sized),
    types: &TypeStore,
    opaque: TypeId,
    witness: TypeId,
) -> bool {
    matches!(types.get(opaque), Some(TypeKind::Opaque { .. }))
        && matches!(
            environment.type_representation(opaque),
            Some(RuntimeTypeRepresentation::Opaque { witness: expected }) if *expected == witness
        )
}

pub(crate) fn matches_opaque_projection(
    environment: &(impl MirValidationEnvironment + ?Sized),
    types: &TypeStore,
    opaque: TypeId,
    definition: OpaqueTypeId,
    witness: TypeId,
) -> bool {
    matches!(
        types.get(opaque),
        Some(TypeKind::Opaque {
            definition: actual,
            ..
        }) if *actual == definition
    ) && matches_opaque_witness(environment, types, opaque, witness)
}
