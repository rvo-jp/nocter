use std::collections::HashMap;

use nocter_model::{BorrowCapability, BuiltinType, ModuleId, NominalTypeId, TypeId, TypeKind};

use crate::{
    BuiltinAttachment, CallableKind, CallableOwner, DeclarationProgram, LiteralShape, NominalShape,
    ParameterRole, Visibility,
};

use super::{
    DeclarationDomain, DeclarationRule, DeclarationViolation, ProgramIntegrityError,
    ProgramValidationError, require,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum AttachmentTarget {
    Nominal(NominalTypeId),
    Builtin(BuiltinType),
    Slice,
}

pub(super) fn validate_ownership(
    program: &DeclarationProgram,
) -> Result<(), ProgramIntegrityError> {
    validate_owned_declaration_sites(program)
}

pub(super) fn validate_rules(program: &DeclarationProgram) -> Result<(), ProgramValidationError> {
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
                    return Err(ProgramIntegrityError::InvalidDeclarationShape(
                        DeclarationDomain::Callable,
                    ));
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

fn validate_primitives(program: &DeclarationProgram) -> Result<(), ProgramValidationError> {
    for (_, callable) in program.declarations().callables().iter() {
        if callable.kind() != CallableKind::Primitive {
            continue;
        }
        let module = site_module(program, callable.site(), DeclarationDomain::Callable)?;
        if !is_standard_package_module(program, module) {
            return violation(DeclarationRule::PrimitiveAuthority, callable.site());
        }
    }
    Ok(())
}

fn validate_constructions(program: &DeclarationProgram) -> Result<(), ProgramValidationError> {
    let mut targets = HashMap::new();
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
            construction.site(),
            DeclarationDomain::Construction,
        )?;
        if let Some(previous) = targets.insert(target, construction.site()) {
            return related_violation(
                DeclarationRule::DuplicateConstruction,
                construction.site(),
                previous,
            );
        }
        for member in construction.members() {
            let member = require(
                program.declarations().callables().get(*member),
                DeclarationDomain::Construction,
                DeclarationDomain::Callable,
            )?;
            if outcome_payload(program, member.result()) != Some(construction.target()) {
                return related_violation(
                    DeclarationRule::InvalidConstructionResult,
                    member.site(),
                    construction.site(),
                );
            }
            if let CallableKind::Literal(shape) = member.kind()
                && !valid_literal_signature(program, member, construction.target(), shape)?
            {
                return related_violation(
                    DeclarationRule::InvalidLiteralSignature,
                    member.site(),
                    construction.site(),
                );
            }
        }
    }
    Ok(())
}

fn valid_literal_signature(
    program: &DeclarationProgram,
    callable: &crate::CallableDeclaration,
    target: TypeId,
    shape: LiteralShape,
) -> Result<bool, ProgramIntegrityError> {
    let site = require(
        program.declaration_sites().get(callable.site()),
        DeclarationDomain::Callable,
        DeclarationDomain::DeclarationSite,
    )?;
    let [parameter] = callable.parameters() else {
        return Ok(false);
    };
    let parameter = require(
        program.declarations().parameters().get(*parameter),
        DeclarationDomain::Callable,
        DeclarationDomain::Parameter,
    )?;
    if site.visibility() != Visibility::Public || callable.result() != target {
        return Ok(false);
    }
    Ok(match shape {
        LiteralShape::Sequence => {
            parameter.role()
                == (ParameterRole::Ordinary {
                    position: 0,
                    variadic: true,
                })
        }
        LiteralShape::String => {
            parameter.role()
                == (ParameterRole::Ordinary {
                    position: 0,
                    variadic: false,
                })
                && matches!(
                    program.types().get(parameter.ty()),
                    Some(TypeKind::Borrow {
                        capability: BorrowCapability::Readonly,
                        referent,
                    }) if *referent == program.types().builtin(BuiltinType::Str)
                )
        }
    })
}

fn validate_instances(program: &DeclarationProgram) -> Result<(), ProgramValidationError> {
    for (_, instance) in program.declarations().instances().iter() {
        let module = site_module(program, instance.site(), DeclarationDomain::Instance)?;
        require_inherent_target(
            program,
            instance.target(),
            module,
            instance.site(),
            DeclarationDomain::Instance,
        )?;
    }
    Ok(())
}

fn validate_conformances(program: &DeclarationProgram) -> Result<(), ProgramValidationError> {
    for (_, conformance) in program.declarations().conformances().iter() {
        let module = site_module(program, conformance.site(), DeclarationDomain::Conformance)?;
        match attachment_target(program, conformance.target()) {
            Some(AttachmentTarget::Nominal(_)) => {}
            Some(AttachmentTarget::Builtin(_) | AttachmentTarget::Slice) => {
                if !is_standard_package_module(program, module) {
                    return violation(
                        DeclarationRule::BuiltinConformanceAuthority,
                        conformance.site(),
                    );
                }
            }
            None => {
                return violation(
                    DeclarationRule::InvalidConformanceTarget,
                    conformance.site(),
                );
            }
        }
    }
    Ok(())
}

fn validate_drops(program: &DeclarationProgram) -> Result<(), ProgramValidationError> {
    let mut targets = HashMap::new();
    for (_, drop) in program.declarations().drops().iter() {
        let module = site_module(program, drop.site(), DeclarationDomain::Drop)?;
        let Some(AttachmentTarget::Nominal(definition)) = attachment_target(program, drop.target())
        else {
            return violation(DeclarationRule::InvalidDropTarget, drop.site());
        };
        let nominal = require(
            program.declarations().nominal_types().get(definition),
            DeclarationDomain::Drop,
            DeclarationDomain::NominalType,
        )?;
        if site_module(program, nominal.site(), DeclarationDomain::NominalType)? != module {
            return related_violation(
                DeclarationRule::InvalidDropTarget,
                drop.site(),
                nominal.site(),
            );
        }
        match nominal.shape() {
            NominalShape::Struct {
                copy_declared: true,
                ..
            } => {
                return related_violation(DeclarationRule::CopyDrop, drop.site(), nominal.site());
            }
            NominalShape::Enum { variants }
                if !variants.iter().any(|variant| {
                    program
                        .declarations()
                        .variants()
                        .get(*variant)
                        .is_some_and(|variant| !variant.payload().is_empty())
                }) =>
            {
                return related_violation(
                    DeclarationRule::PayloadlessEnumDrop,
                    drop.site(),
                    nominal.site(),
                );
            }
            NominalShape::Struct {
                copy_declared: false,
                ..
            }
            | NominalShape::Enum { .. } => {}
        }
        if let Some(previous) = targets.insert(definition, drop.site()) {
            return related_violation(DeclarationRule::DuplicateDrop, drop.site(), previous);
        }
    }
    Ok(())
}

fn require_inherent_target(
    program: &DeclarationProgram,
    ty: TypeId,
    module: ModuleId,
    site: nocter_model::DeclarationSiteId,
    domain: DeclarationDomain,
) -> Result<AttachmentTarget, ProgramValidationError> {
    let Some(target) = attachment_target(program, ty) else {
        return violation(DeclarationRule::InvalidInherentAttachment, site);
    };
    match target {
        AttachmentTarget::Nominal(definition) => {
            let nominal = require(
                program.declarations().nominal_types().get(definition),
                domain,
                DeclarationDomain::NominalType,
            )?;
            if site_module(program, nominal.site(), DeclarationDomain::NominalType)? != module {
                return related_violation(
                    DeclarationRule::InvalidInherentAttachment,
                    site,
                    nominal.site(),
                );
            }
        }
        AttachmentTarget::Builtin(builtin) => {
            let Some(attachment) = builtin_attachment(builtin) else {
                return violation(DeclarationRule::InvalidInherentAttachment, site);
            };
            if !is_standard_attachment_module(program, module, attachment) {
                return violation(DeclarationRule::InvalidInherentAttachment, site);
            }
        }
        AttachmentTarget::Slice => {
            if !is_standard_attachment_module(program, module, BuiltinAttachment::Slice) {
                return violation(DeclarationRule::InvalidInherentAttachment, site);
            }
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

fn is_standard_attachment_module(
    program: &DeclarationProgram,
    module: ModuleId,
    attachment: BuiltinAttachment,
) -> bool {
    program
        .standard_library()
        .and_then(|standard| standard.attachment_module(attachment))
        == Some(module)
}

fn is_standard_package_module(program: &DeclarationProgram, module: ModuleId) -> bool {
    let package = program.modules().get(module).map(crate::Module::package);
    package.is_some() && package == program.standard_package()
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

fn violation<T>(
    rule: DeclarationRule,
    primary: nocter_model::DeclarationSiteId,
) -> Result<T, ProgramValidationError> {
    Err(DeclarationViolation::new(rule, primary).into())
}

fn related_violation<T>(
    rule: DeclarationRule,
    primary: nocter_model::DeclarationSiteId,
    related: nocter_model::DeclarationSiteId,
) -> Result<T, ProgramValidationError> {
    Err(DeclarationViolation::with_related(rule, primary, related).into())
}
