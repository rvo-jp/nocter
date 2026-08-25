use std::collections::HashMap;

use nocter_model::ModuleId;

use super::attachment_rules::{
    AttachmentTarget, attachment_target, conformance_target_is_authorized,
    inherent_target_is_authorized, is_standard_package_module, outcome_payload,
    valid_literal_signature,
};
use crate::{CallableKind, CallableOwner, DeclarationProgram, NominalShape};

use super::outcome::{ValidationCollector, related_violation, violation};
use super::{DeclarationDomain, DeclarationRule, ProgramIntegrityError, require};

pub(super) fn validate_ownership(
    program: &DeclarationProgram,
) -> Result<(), ProgramIntegrityError> {
    validate_owned_declaration_sites(program)
}

pub(super) fn validate_rules(
    program: &DeclarationProgram,
    collector: &mut ValidationCollector,
) -> Result<(), ProgramIntegrityError> {
    validate_primitives(program, collector)?;
    validate_constructions(program, collector)?;
    validate_instances(program, collector)?;
    validate_conformances(program, collector)?;
    validate_drops(program, collector)
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

fn validate_primitives(
    program: &DeclarationProgram,
    collector: &mut ValidationCollector,
) -> Result<(), ProgramIntegrityError> {
    for (_, callable) in program.declarations().callables().iter() {
        if callable.kind() == CallableKind::Primitive {
            let module = site_module(program, callable.site(), DeclarationDomain::Callable)?;
            if !is_standard_package_module(program, module) {
                collector.report(violation(
                    DeclarationRule::PrimitiveAuthority,
                    callable.site(),
                ));
            }
        }
    }
    Ok(())
}

fn validate_constructions(
    program: &DeclarationProgram,
    collector: &mut ValidationCollector,
) -> Result<(), ProgramIntegrityError> {
    let mut targets = HashMap::<AttachmentTarget, Vec<_>>::new();
    for (id, construction) in program.declarations().constructions().iter() {
        let module = site_module(
            program,
            construction.site(),
            DeclarationDomain::Construction,
        )?;
        let target = attachment_target(program, construction.target());
        let authorized = inherent_target_is_authorized(program, construction.target(), module);
        if !authorized {
            let related = match target {
                Some(AttachmentTarget::Nominal(definition)) => Some(
                    require(
                        program.declarations().nominal_types().get(definition),
                        DeclarationDomain::Construction,
                        DeclarationDomain::NominalType,
                    )?
                    .site(),
                ),
                _ => None,
            };
            let error = related.map_or_else(
                || {
                    violation(
                        DeclarationRule::InvalidInherentAttachment,
                        construction.site(),
                    )
                },
                |site| {
                    related_violation(
                        DeclarationRule::InvalidInherentAttachment,
                        construction.site(),
                        site,
                    )
                },
            );
            collector.reject_construction(id, error);
        } else if let Some(target) = target {
            targets
                .entry(target)
                .or_default()
                .push((id, construction.site()));
        }
        for member in construction.members() {
            let member = require(
                program.declarations().callables().get(*member),
                DeclarationDomain::Construction,
                DeclarationDomain::Callable,
            )?;
            if outcome_payload(program, member.result()) != Some(construction.target()) {
                collector.reject_construction(
                    id,
                    related_violation(
                        DeclarationRule::InvalidConstructionResult,
                        member.site(),
                        construction.site(),
                    ),
                );
            }
            if let CallableKind::Literal(shape) = member.kind()
                && !valid_literal_signature(program, member, construction.target(), shape)
            {
                collector.reject_construction(
                    id,
                    related_violation(
                        DeclarationRule::InvalidLiteralSignature,
                        member.site(),
                        construction.site(),
                    ),
                );
            }
        }
    }
    for declarations in targets
        .values()
        .filter(|declarations| declarations.len() > 1)
    {
        let previous = declarations[0].1;
        collector.quarantine_construction(declarations[0].0);
        for (id, site) in &declarations[1..] {
            collector.reject_construction(
                *id,
                related_violation(DeclarationRule::DuplicateConstruction, *site, previous),
            );
        }
    }
    Ok(())
}

fn validate_instances(
    program: &DeclarationProgram,
    collector: &mut ValidationCollector,
) -> Result<(), ProgramIntegrityError> {
    for (id, instance) in program.declarations().instances().iter() {
        let module = site_module(program, instance.site(), DeclarationDomain::Instance)?;
        if !inherent_target_is_authorized(program, instance.target(), module) {
            let related = match attachment_target(program, instance.target()) {
                Some(AttachmentTarget::Nominal(definition)) => Some(
                    require(
                        program.declarations().nominal_types().get(definition),
                        DeclarationDomain::Instance,
                        DeclarationDomain::NominalType,
                    )?
                    .site(),
                ),
                _ => None,
            };
            let error = related.map_or_else(
                || violation(DeclarationRule::InvalidInherentAttachment, instance.site()),
                |site| {
                    related_violation(
                        DeclarationRule::InvalidInherentAttachment,
                        instance.site(),
                        site,
                    )
                },
            );
            collector.reject_instance(id, error);
        }
    }
    Ok(())
}

fn validate_conformances(
    program: &DeclarationProgram,
    collector: &mut ValidationCollector,
) -> Result<(), ProgramIntegrityError> {
    for (id, conformance) in program.declarations().conformances().iter() {
        let module = site_module(program, conformance.site(), DeclarationDomain::Conformance)?;
        match attachment_target(program, conformance.target()) {
            Some(AttachmentTarget::Builtin(_) | AttachmentTarget::Slice)
                if !conformance_target_is_authorized(program, conformance.target(), module) =>
            {
                collector.reject_conformance(
                    id,
                    violation(
                        DeclarationRule::BuiltinConformanceAuthority,
                        conformance.site(),
                    ),
                );
            }
            None => collector.reject_conformance(
                id,
                violation(
                    DeclarationRule::InvalidConformanceTarget,
                    conformance.site(),
                ),
            ),
            Some(
                AttachmentTarget::Nominal(_)
                | AttachmentTarget::Builtin(_)
                | AttachmentTarget::Slice,
            ) => {}
        }
    }
    Ok(())
}

fn validate_drops(
    program: &DeclarationProgram,
    collector: &mut ValidationCollector,
) -> Result<(), ProgramIntegrityError> {
    let mut targets = HashMap::<nocter_model::NominalTypeId, Vec<_>>::new();
    for (id, drop) in program.declarations().drops().iter() {
        let module = site_module(program, drop.site(), DeclarationDomain::Drop)?;
        let Some(AttachmentTarget::Nominal(definition)) = attachment_target(program, drop.target())
        else {
            collector.reject_drop(
                id,
                violation(DeclarationRule::InvalidDropTarget, drop.site()),
            );
            continue;
        };
        let nominal = require(
            program.declarations().nominal_types().get(definition),
            DeclarationDomain::Drop,
            DeclarationDomain::NominalType,
        )?;
        let owned = site_module(program, nominal.site(), DeclarationDomain::NominalType)? == module;
        if !owned {
            collector.reject_drop(
                id,
                related_violation(
                    DeclarationRule::InvalidDropTarget,
                    drop.site(),
                    nominal.site(),
                ),
            );
        }
        match nominal.shape() {
            NominalShape::Struct {
                copy_declared: true,
                ..
            } => collector.reject_drop(
                id,
                related_violation(DeclarationRule::CopyDrop, drop.site(), nominal.site()),
            ),
            NominalShape::Enum { variants }
                if !variants.iter().any(|variant| {
                    program
                        .declarations()
                        .variants()
                        .get(*variant)
                        .is_some_and(|variant| !variant.payload().is_empty())
                }) =>
            {
                collector.reject_drop(
                    id,
                    related_violation(
                        DeclarationRule::PayloadlessEnumDrop,
                        drop.site(),
                        nominal.site(),
                    ),
                );
            }
            NominalShape::Struct {
                copy_declared: false,
                ..
            }
            | NominalShape::Enum { .. } => {}
        }
        if owned {
            targets
                .entry(definition)
                .or_default()
                .push((id, drop.site()));
        }
    }
    for declarations in targets
        .values()
        .filter(|declarations| declarations.len() > 1)
    {
        let previous = declarations[0].1;
        collector.quarantine_drop(declarations[0].0);
        for (id, site) in &declarations[1..] {
            collector.reject_drop(
                *id,
                related_violation(DeclarationRule::DuplicateDrop, *site, previous),
            );
        }
    }
    Ok(())
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
