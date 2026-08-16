use std::collections::HashSet;

use nocter_model::RequirementId;

use crate::{
    AssociatedTypeBinding, DeclarationProgram, RequirementKind, RequirementOwner,
    RequirementSubject, StructuralCapability,
};

use super::{DeclarationDomain, ProgramIntegrityError, require, require_type};

pub(super) fn validate(program: &DeclarationProgram) -> Result<(), ProgramIntegrityError> {
    for (id, requirement) in program.declarations().requirements().iter() {
        if !owner_contains(program, requirement.owner(), id) {
            return Err(ProgramIntegrityError::OwnerMismatch(
                DeclarationDomain::Requirement,
            ));
        }
        match requirement.kind() {
            RequirementKind::Capability {
                subject,
                capability,
            } => {
                validate_subject(program, *subject)?;
                match capability {
                    StructuralCapability::Interface(interface) => validate_interface_application(
                        program,
                        interface,
                        DeclarationDomain::Requirement,
                    )?,
                    StructuralCapability::Callable(contract) => {
                        for parameter in contract.parameters() {
                            require_type(program, *parameter, DeclarationDomain::Requirement)?;
                        }
                        require_type(program, contract.result(), DeclarationDomain::Requirement)?;
                    }
                }
            }
            RequirementKind::Copy(parameter)
            | RequirementKind::Equality { operand: parameter }
            | RequirementKind::Ordering { operand: parameter } => {
                require(
                    program.declarations().generic_parameters().get(*parameter),
                    DeclarationDomain::Requirement,
                    DeclarationDomain::GenericParameter,
                )?;
            }
            RequirementKind::TypeEquality { left, right } => {
                require_type(program, *left, DeclarationDomain::Requirement)?;
                require_type(program, *right, DeclarationDomain::Requirement)?;
            }
            RequirementKind::Index {
                container,
                index,
                result,
                ..
            } => {
                require(
                    program.declarations().generic_parameters().get(*container),
                    DeclarationDomain::Requirement,
                    DeclarationDomain::GenericParameter,
                )?;
                require_type(program, *index, DeclarationDomain::Requirement)?;
                require_type(program, *result, DeclarationDomain::Requirement)?;
            }
            RequirementKind::Coercion { source, target } => {
                require_type(program, *source, DeclarationDomain::Requirement)?;
                require_type(program, *target, DeclarationDomain::Requirement)?;
            }
            RequirementKind::Expansion { source, result, .. } => {
                require(
                    program.declarations().generic_parameters().get(*source),
                    DeclarationDomain::Requirement,
                    DeclarationDomain::GenericParameter,
                )?;
                require_type(program, *result, DeclarationDomain::Requirement)?;
            }
            RequirementKind::BinderRefinement {
                parameter,
                replacement,
            } => {
                require(
                    program.declarations().generic_parameters().get(*parameter),
                    DeclarationDomain::Requirement,
                    DeclarationDomain::GenericParameter,
                )?;
                require_type(program, *replacement, DeclarationDomain::Requirement)?;
            }
        }
    }
    Ok(())
}

fn validate_subject(
    program: &DeclarationProgram,
    subject: RequirementSubject,
) -> Result<(), ProgramIntegrityError> {
    match subject {
        RequirementSubject::GenericParameter(parameter) => require(
            program.declarations().generic_parameters().get(parameter),
            DeclarationDomain::Requirement,
            DeclarationDomain::GenericParameter,
        )
        .map(|_| ()),
        RequirementSubject::AssociatedType(associated) => require(
            program.declarations().associated_types().get(associated),
            DeclarationDomain::Requirement,
            DeclarationDomain::AssociatedType,
        )
        .map(|_| ()),
    }
}

fn owner_contains(
    program: &DeclarationProgram,
    owner: RequirementOwner,
    requirement: RequirementId,
) -> bool {
    let declarations = program.declarations();
    match owner {
        RequirementOwner::NominalType(owner) => declarations
            .nominal_types()
            .get(owner)
            .is_some_and(|owner| owner.requirements().contains(&requirement)),
        RequirementOwner::TypeAlias(owner) => declarations
            .type_aliases()
            .get(owner)
            .is_some_and(|owner| owner.requirements().contains(&requirement)),
        RequirementOwner::Interface(owner) => {
            declarations.interfaces().get(owner).is_some_and(|owner| {
                owner.requirements().contains(&requirement)
                    || owner.associated_types().iter().any(|associated| {
                        declarations
                            .associated_types()
                            .get(*associated)
                            .is_some_and(|associated| associated.bounds().contains(&requirement))
                    })
            })
        }
        RequirementOwner::Callable(owner) => declarations
            .callables()
            .get(owner)
            .is_some_and(|owner| owner.requirements().contains(&requirement)),
        RequirementOwner::Instance(owner) => declarations
            .instances()
            .get(owner)
            .is_some_and(|owner| owner.requirements().contains(&requirement)),
        RequirementOwner::Conformance(owner) => declarations
            .conformances()
            .get(owner)
            .is_some_and(|owner| owner.requirements().contains(&requirement)),
    }
}

pub(super) fn validate_associated_bindings(
    program: &DeclarationProgram,
    bindings: &[AssociatedTypeBinding],
    interface: nocter_model::InterfaceId,
    owner: DeclarationDomain,
) -> Result<(), ProgramIntegrityError> {
    let mut seen = HashSet::new();
    for binding in bindings {
        if !seen.insert(binding.declaration()) {
            return Err(ProgramIntegrityError::DuplicateReference(owner));
        }
        let declaration = require(
            program
                .declarations()
                .associated_types()
                .get(binding.declaration()),
            owner,
            DeclarationDomain::AssociatedType,
        )?;
        if declaration.interface() != interface {
            return Err(ProgramIntegrityError::OwnerMismatch(owner));
        }
        require_type(program, binding.ty(), owner)?;
    }
    Ok(())
}

pub(super) fn validate_interface_application(
    program: &DeclarationProgram,
    application: &crate::InterfaceApplication,
    owner: DeclarationDomain,
) -> Result<(), ProgramIntegrityError> {
    let declaration = require(
        program
            .declarations()
            .interfaces()
            .get(application.interface()),
        owner,
        DeclarationDomain::Interface,
    )?;
    if application.arguments().len() != declaration.generic_parameters().len() {
        return Err(ProgramIntegrityError::InvalidPosition(owner));
    }
    for argument in application.arguments() {
        require_type(program, *argument, owner)?;
    }
    Ok(())
}
