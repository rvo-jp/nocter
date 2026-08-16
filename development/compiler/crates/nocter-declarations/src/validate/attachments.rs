use std::collections::HashSet;

use nocter_model::{BuiltinType, ModuleId, NominalTypeId, TypeId, TypeKind};

use crate::{BuiltinAttachment, CallableKind, CallableOwner, DeclarationProgram, NominalShape};

use super::{DeclarationDomain, ProgramIntegrityError, require};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum AttachmentTarget {
    Nominal(NominalTypeId),
    Builtin(BuiltinType),
    Slice,
}

pub(super) fn validate(program: &DeclarationProgram) -> Result<(), ProgramIntegrityError> {
    validate_owned_declaration_sites(program)?;
    validate_primitives(program)?;
    validate_constructions(program)?;
    validate_instances(program)?;
    validate_conformances(program)?;
    validate_drops(program)
}

fn validate_owned_declaration_sites(
    program: &DeclarationProgram,
) -> Result<(), ProgramIntegrityError> {
    let declarations = program.declarations();
    for (_, field) in declarations.fields().iter() {
        require_same_site_module(
            program,
            field.site(),
            declarations
                .nominal_types()
                .get(field.owner())
                .map(crate::NominalTypeDeclaration::site),
            DeclarationDomain::Field,
        )?;
    }
    for (_, variant) in declarations.variants().iter() {
        require_same_site_module(
            program,
            variant.site(),
            declarations
                .nominal_types()
                .get(variant.owner())
                .map(crate::NominalTypeDeclaration::site),
            DeclarationDomain::Variant,
        )?;
    }
    for (_, associated) in declarations.associated_types().iter() {
        require_same_site_module(
            program,
            associated.site(),
            declarations
                .interfaces()
                .get(associated.interface())
                .map(crate::InterfaceDeclaration::site),
            DeclarationDomain::AssociatedType,
        )?;
    }
    for (_, callable) in declarations.callables().iter() {
        let owner_site = match callable.owner() {
            CallableOwner::Module(module) => {
                if site_module(program, callable.site(), DeclarationDomain::Callable)? != module {
                    return invalid(DeclarationDomain::Callable);
                }
                continue;
            }
            CallableOwner::Construction(owner) => declarations
                .constructions()
                .get(owner)
                .map(crate::ConstructionDeclaration::site),
            CallableOwner::Instance(owner) => declarations
                .instances()
                .get(owner)
                .map(crate::InstanceDeclaration::site),
            CallableOwner::Interface(owner) => declarations
                .interfaces()
                .get(owner)
                .map(crate::InterfaceDeclaration::site),
            CallableOwner::Conformance(owner) => declarations
                .conformances()
                .get(owner)
                .map(crate::ConformanceDeclaration::site),
        };
        require_same_site_module(
            program,
            callable.site(),
            owner_site,
            DeclarationDomain::Callable,
        )?;
    }
    Ok(())
}

fn validate_primitives(program: &DeclarationProgram) -> Result<(), ProgramIntegrityError> {
    for (_, callable) in program.declarations().callables().iter() {
        if callable.kind() != CallableKind::Primitive {
            continue;
        }
        let module = site_module(program, callable.site(), DeclarationDomain::Callable)?;
        require_standard_package_module(program, module, DeclarationDomain::Callable)?;
    }
    Ok(())
}

fn validate_constructions(program: &DeclarationProgram) -> Result<(), ProgramIntegrityError> {
    let mut targets = HashSet::new();
    for (_, construction) in program.declarations().constructions().iter() {
        let module = site_module(
            program,
            construction.site(),
            DeclarationDomain::Construction,
        )?;
        let target = require_inherent_target(
            program,
            construction.target(),
            module,
            DeclarationDomain::Construction,
        )?;
        if !targets.insert(target) {
            return Err(ProgramIntegrityError::DuplicateReference(
                DeclarationDomain::Construction,
            ));
        }
        for member in construction.members() {
            let member = require(
                program.declarations().callables().get(*member),
                DeclarationDomain::Construction,
                DeclarationDomain::Callable,
            )?;
            if outcome_payload(program, member.result()) != Some(construction.target()) {
                return invalid(DeclarationDomain::Construction);
            }
        }
    }
    Ok(())
}

fn validate_instances(program: &DeclarationProgram) -> Result<(), ProgramIntegrityError> {
    for (_, instance) in program.declarations().instances().iter() {
        let module = site_module(program, instance.site(), DeclarationDomain::Instance)?;
        require_inherent_target(
            program,
            instance.target(),
            module,
            DeclarationDomain::Instance,
        )?;
    }
    Ok(())
}

fn validate_conformances(program: &DeclarationProgram) -> Result<(), ProgramIntegrityError> {
    for (_, conformance) in program.declarations().conformances().iter() {
        let module = site_module(program, conformance.site(), DeclarationDomain::Conformance)?;
        match attachment_target(program, conformance.target()) {
            Some(AttachmentTarget::Nominal(_)) => {}
            Some(AttachmentTarget::Builtin(_) | AttachmentTarget::Slice) => {
                require_standard_package_module(program, module, DeclarationDomain::Conformance)?;
            }
            None => return invalid(DeclarationDomain::Conformance),
        }
    }
    Ok(())
}

fn validate_drops(program: &DeclarationProgram) -> Result<(), ProgramIntegrityError> {
    let mut targets = HashSet::new();
    for (_, drop) in program.declarations().drops().iter() {
        let module = site_module(program, drop.site(), DeclarationDomain::Drop)?;
        let Some(AttachmentTarget::Nominal(definition)) = attachment_target(program, drop.target())
        else {
            return invalid(DeclarationDomain::Drop);
        };
        let nominal = require(
            program.declarations().nominal_types().get(definition),
            DeclarationDomain::Drop,
            DeclarationDomain::NominalType,
        )?;
        if site_module(program, nominal.site(), DeclarationDomain::NominalType)? != module
            || !drop_shape_can_own_body(program, nominal.shape())
        {
            return invalid(DeclarationDomain::Drop);
        }
        if !targets.insert(definition) {
            return Err(ProgramIntegrityError::DuplicateReference(
                DeclarationDomain::Drop,
            ));
        }
    }
    Ok(())
}

fn require_inherent_target(
    program: &DeclarationProgram,
    ty: TypeId,
    module: ModuleId,
    domain: DeclarationDomain,
) -> Result<AttachmentTarget, ProgramIntegrityError> {
    let target = attachment_target(program, ty)
        .ok_or(ProgramIntegrityError::InvalidDeclarationShape(domain))?;
    match target {
        AttachmentTarget::Nominal(definition) => {
            let nominal = require(
                program.declarations().nominal_types().get(definition),
                domain,
                DeclarationDomain::NominalType,
            )?;
            if site_module(program, nominal.site(), DeclarationDomain::NominalType)? != module {
                return invalid(domain);
            }
        }
        AttachmentTarget::Builtin(builtin) => {
            let attachment = builtin_attachment(builtin)
                .ok_or(ProgramIntegrityError::InvalidDeclarationShape(domain))?;
            require_standard_attachment_module(program, module, attachment, domain)?;
        }
        AttachmentTarget::Slice => {
            require_standard_attachment_module(program, module, BuiltinAttachment::Slice, domain)?;
        }
    }
    Ok(target)
}

fn attachment_target(program: &DeclarationProgram, ty: TypeId) -> Option<AttachmentTarget> {
    match program.types().get(ty)? {
        TypeKind::Nominal { definition, .. } => Some(AttachmentTarget::Nominal(*definition)),
        TypeKind::Builtin(builtin) => Some(AttachmentTarget::Builtin(*builtin)),
        TypeKind::Slice(_) => Some(AttachmentTarget::Slice),
        _ => None,
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

fn require_standard_attachment_module(
    program: &DeclarationProgram,
    module: ModuleId,
    attachment: BuiltinAttachment,
    domain: DeclarationDomain,
) -> Result<(), ProgramIntegrityError> {
    if program
        .standard_library()
        .and_then(|standard| standard.attachment_module(attachment))
        == Some(module)
    {
        Ok(())
    } else {
        invalid(domain)
    }
}

fn require_standard_package_module(
    program: &DeclarationProgram,
    module: ModuleId,
    domain: DeclarationDomain,
) -> Result<(), ProgramIntegrityError> {
    let package = program.modules().get(module).map(crate::Module::package);
    if package.is_some() && package == program.standard_package() {
        Ok(())
    } else {
        invalid(domain)
    }
}

fn drop_shape_can_own_body(program: &DeclarationProgram, shape: &NominalShape) -> bool {
    match shape {
        NominalShape::Struct { copy_declared, .. } => !copy_declared,
        NominalShape::Enum { variants } => variants.iter().any(|variant| {
            program
                .declarations()
                .variants()
                .get(*variant)
                .is_some_and(|variant| !variant.payload().is_empty())
        }),
    }
}

fn outcome_payload(program: &DeclarationProgram, mut ty: TypeId) -> Option<TypeId> {
    loop {
        match program.types().get(ty)? {
            TypeKind::Optional(payload) | TypeKind::Fallible(payload) => ty = *payload,
            _ => return Some(ty),
        }
    }
}

fn require_same_site_module(
    program: &DeclarationProgram,
    site: nocter_model::DeclarationSiteId,
    owner_site: Option<nocter_model::DeclarationSiteId>,
    domain: DeclarationDomain,
) -> Result<(), ProgramIntegrityError> {
    let owner_site = owner_site.ok_or(ProgramIntegrityError::UnknownReference {
        owner: domain,
        target: DeclarationDomain::DeclarationSite,
    })?;
    if site_module(program, site, domain)? == site_module(program, owner_site, domain)? {
        Ok(())
    } else {
        Err(ProgramIntegrityError::OwnerMismatch(domain))
    }
}

fn site_module(
    program: &DeclarationProgram,
    site: nocter_model::DeclarationSiteId,
    domain: DeclarationDomain,
) -> Result<ModuleId, ProgramIntegrityError> {
    require(
        program.declaration_sites().get(site),
        domain,
        DeclarationDomain::DeclarationSite,
    )
    .map(|site| site.module())
}

fn invalid<T>(domain: DeclarationDomain) -> Result<T, ProgramIntegrityError> {
    Err(ProgramIntegrityError::InvalidDeclarationShape(domain))
}
