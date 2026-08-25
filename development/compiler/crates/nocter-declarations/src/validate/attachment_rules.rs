use nocter_model::{BorrowCapability, BuiltinType, ModuleId, TypeId, TypeKind};

use crate::{
    BuiltinAttachment, CallableDeclaration, DeclarationProgram, LiteralShape, ParameterRole,
    Visibility,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(super) enum AttachmentTarget {
    Nominal(nocter_model::NominalTypeId),
    Builtin(BuiltinType),
    Slice,
}

pub(super) fn outcome_payload(program: &DeclarationProgram, mut ty: TypeId) -> Option<TypeId> {
    loop {
        match program.types().get(ty)? {
            TypeKind::Optional(payload) | TypeKind::Fallible(payload) => ty = *payload,
            _ => return Some(ty),
        }
    }
}

pub(super) fn valid_literal_signature(
    program: &DeclarationProgram,
    callable: &CallableDeclaration,
    target: TypeId,
    shape: LiteralShape,
) -> bool {
    let Some(site) = program.graph().declaration_sites().get(callable.site()) else {
        return false;
    };
    let [parameter] = callable.parameters() else {
        return false;
    };
    let Some(parameter) = program.graph().declarations().parameters().get(*parameter) else {
        return false;
    };
    if site.visibility() != Visibility::Public || callable.result() != target {
        return false;
    }
    match shape {
        LiteralShape::Sequence => parameter.role() == ParameterRole::ArgumentPack { position: 0 },
        LiteralShape::String => {
            parameter.role() == ParameterRole::Ordinary { position: 0 }
                && matches!(
                    program.types().get(parameter.ty()),
                    Some(TypeKind::Borrow {
                        capability: BorrowCapability::Readonly,
                        referent,
                    }) if *referent == program.types().builtin(BuiltinType::Str)
                )
        }
    }
}

pub(super) fn attachment_target(
    program: &DeclarationProgram,
    ty: TypeId,
) -> Option<AttachmentTarget> {
    match program.types().get(ty)? {
        TypeKind::Nominal { definition, .. } => Some(AttachmentTarget::Nominal(*definition)),
        TypeKind::Builtin(builtin) => Some(AttachmentTarget::Builtin(*builtin)),
        TypeKind::Slice(_) => Some(AttachmentTarget::Slice),
        _ => None,
    }
}

pub(super) fn inherent_target_is_authorized(
    program: &DeclarationProgram,
    ty: TypeId,
    module: ModuleId,
) -> bool {
    match attachment_target(program, ty) {
        Some(AttachmentTarget::Nominal(definition)) => program
            .graph()
            .declarations()
            .nominal_types()
            .get(definition)
            .and_then(|declaration| program.graph().declaration_sites().get(declaration.site()))
            .is_some_and(|site| site.module() == module),
        Some(AttachmentTarget::Builtin(builtin)) => builtin_attachment(builtin)
            .is_some_and(|attachment| is_standard_attachment_module(program, module, attachment)),
        Some(AttachmentTarget::Slice) => {
            is_standard_attachment_module(program, module, BuiltinAttachment::Slice)
        }
        None => false,
    }
}

pub(super) fn conformance_target_is_authorized(
    program: &DeclarationProgram,
    ty: TypeId,
    module: ModuleId,
) -> bool {
    match attachment_target(program, ty) {
        Some(AttachmentTarget::Nominal(_)) => true,
        Some(AttachmentTarget::Builtin(_) | AttachmentTarget::Slice) => {
            is_standard_package_module(program, module)
        }
        None => false,
    }
}

fn builtin_attachment(builtin: BuiltinType) -> Option<BuiltinAttachment> {
    match builtin {
        BuiltinType::Bool
        | BuiltinType::I8
        | BuiltinType::I16
        | BuiltinType::I32
        | BuiltinType::I64
        | BuiltinType::U8
        | BuiltinType::U16
        | BuiltinType::U32
        | BuiltinType::U64
        | BuiltinType::Usize
        | BuiltinType::Isize => Some(BuiltinAttachment::Scalar),
        BuiltinType::Str => Some(BuiltinAttachment::Str),
        BuiltinType::Error => Some(BuiltinAttachment::Error),
        BuiltinType::Void | BuiltinType::Never => None,
    }
}

fn is_standard_attachment_module(
    program: &DeclarationProgram,
    module: ModuleId,
    attachment: BuiltinAttachment,
) -> bool {
    program
        .graph()
        .standard_library()
        .and_then(|standard| standard.attachment_module(attachment))
        == Some(module)
}

pub(super) fn is_standard_package_module(program: &DeclarationProgram, module: ModuleId) -> bool {
    let package = program
        .graph()
        .modules()
        .get(module)
        .map(crate::Module::package);
    package.is_some() && package == program.graph().standard_package()
}
