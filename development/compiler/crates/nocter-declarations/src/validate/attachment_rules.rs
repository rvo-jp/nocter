use nocter_model::{AttachmentFamily, BorrowCapability, BuiltinType, ModuleId, TypeId, TypeKind};
use nocter_toolchain_contract::StructuralAttachment;

use crate::{CallableDeclaration, DeclarationProgram, LiteralShape, ParameterRole, Visibility};

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
    if site.visibility() != Visibility::Public
        || !callable.generic_parameters().is_empty()
        || callable.result() != target
    {
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
) -> Option<AttachmentFamily> {
    AttachmentFamily::of(program.types(), ty)
}

pub(super) fn inherent_target_is_authorized(
    program: &DeclarationProgram,
    ty: TypeId,
    module: ModuleId,
) -> bool {
    match attachment_target(program, ty) {
        Some(AttachmentFamily::Nominal(definition)) => program
            .graph()
            .declarations()
            .nominal_types()
            .get(definition)
            .and_then(|declaration| program.graph().declaration_sites().get(declaration.site()))
            .is_some_and(|site| site.module() == module),
        Some(AttachmentFamily::Builtin(builtin)) => {
            program
                .graph()
                .standard_library()
                .and_then(|standard| standard.builtin_type_module(builtin))
                == Some(module)
        }
        Some(AttachmentFamily::Slice) => {
            is_structural_attachment_module(program, module, StructuralAttachment::Slice)
        }
        None => false,
    }
}

pub(super) fn interface_is_owned_by_module(
    program: &DeclarationProgram,
    interface: nocter_model::InterfaceId,
    module: ModuleId,
) -> bool {
    program
        .declarations()
        .interfaces()
        .get(interface)
        .and_then(|declaration| program.graph().declaration_sites().get(declaration.site()))
        .is_some_and(|site| site.module() == module)
}

fn is_structural_attachment_module(
    program: &DeclarationProgram,
    module: ModuleId,
    attachment: StructuralAttachment,
) -> bool {
    program
        .graph()
        .standard_library()
        .and_then(|standard| standard.structural_attachment_module(attachment))
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
