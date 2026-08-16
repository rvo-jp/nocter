use std::fmt;
use std::{collections::HashSet, hash::Hash};

use nocter_model::{
    CallableCapability, CallableId, GenericParameterId, ModuleId, RequirementId, Symbol, TypeId,
};

use crate::{
    AssociatedTypeBinding, BodyOwner, CallableKind, CallableOwner, DeclarationProgram,
    GenericOwner, ParameterOwner, ParameterRole, ProvenanceOrigin, RequirementKind,
    RequirementOwner, RequirementSubject, StructuralCapability, Visibility,
};

mod graph;
mod types;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeclarationDomain {
    Package,
    Module,
    PackageTarget,
    Import,
    DeclarationSite,
    NominalType,
    TypeAlias,
    Interface,
    AssociatedType,
    Callable,
    Construction,
    Instance,
    Conformance,
    Drop,
    Test,
    Field,
    Variant,
    GenericParameter,
    Parameter,
    Requirement,
    Body,
    OpaqueType,
    Type,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProgramIntegrityError {
    UnknownSymbol(DeclarationDomain),
    UnknownType(DeclarationDomain),
    UnknownReference {
        owner: DeclarationDomain,
        target: DeclarationDomain,
    },
    OwnerMismatch(DeclarationDomain),
    DuplicateReference(DeclarationDomain),
    InvalidPosition(DeclarationDomain),
    InvalidCallableShape,
    InvalidVisibility(DeclarationDomain),
    EmptyImportSelection,
}

impl fmt::Display for ProgramIntegrityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSymbol(owner) => write!(formatter, "{owner:?} contains an unknown symbol"),
            Self::UnknownType(owner) => write!(formatter, "{owner:?} contains an unknown type"),
            Self::UnknownReference { owner, target } => {
                write!(
                    formatter,
                    "{owner:?} contains an unknown {target:?} reference"
                )
            }
            Self::OwnerMismatch(domain) => {
                write!(formatter, "{domain:?} is not owned by its recorded parent")
            }
            Self::DuplicateReference(domain) => {
                write!(formatter, "{domain:?} contains a duplicate identity")
            }
            Self::InvalidPosition(domain) => {
                write!(formatter, "{domain:?} has a non-canonical position")
            }
            Self::InvalidCallableShape => {
                formatter.write_str("callable kind, owner, name, receiver, or body is inconsistent")
            }
            Self::InvalidVisibility(domain) => {
                write!(
                    formatter,
                    "{domain:?} has an invalid normalized visibility boundary"
                )
            }
            Self::EmptyImportSelection => {
                formatter.write_str("resolved selected-name import is empty")
            }
        }
    }
}

impl std::error::Error for ProgramIntegrityError {}

pub(crate) fn validate(program: &DeclarationProgram) -> Result<(), ProgramIntegrityError> {
    types::validate_types(program)?;
    graph::validate_packages_modules_sites(program)?;
    types::validate_nominal_types(program)?;
    types::validate_aliases_interfaces(program)?;
    validate_callables(program)?;
    validate_constructions_instances_conformances(program)?;
    validate_drops_tests(program)?;
    validate_generic_parameters(program)?;
    validate_parameters(program)?;
    validate_requirements(program)?;
    validate_bodies(program)?;
    validate_opaque_types(program)?;
    graph::validate_imports(program)?;
    graph::validate_package_targets(program)
}

fn validate_callables(program: &DeclarationProgram) -> Result<(), ProgramIntegrityError> {
    let declarations = program.declarations();
    for (id, callable) in declarations.callables().iter() {
        require_site(program, callable.site(), DeclarationDomain::Callable)?;
        require_optional_symbol(program, callable.name(), DeclarationDomain::Callable)?;
        require_optional_symbol(program, callable.target_gate(), DeclarationDomain::Callable)?;
        require_type(program, callable.result(), DeclarationDomain::Callable)?;
        unique(callable.generic_parameters(), DeclarationDomain::Callable)?;
        unique(callable.parameters(), DeclarationDomain::Callable)?;
        unique(callable.requirements(), DeclarationDomain::Callable)?;
        validate_callable_shape(callable)?;
        validate_callable_owner(program, id, callable.owner())?;
        if let Some(receiver) = callable.receiver() {
            let parameter = require(
                declarations.parameters().get(receiver),
                DeclarationDomain::Callable,
                DeclarationDomain::Parameter,
            )?;
            if parameter.owner() != ParameterOwner::Callable(id)
                || !matches!(parameter.role(), ParameterRole::Receiver(_))
            {
                return Err(ProgramIntegrityError::OwnerMismatch(
                    DeclarationDomain::Parameter,
                ));
            }
        }
        for origin in callable
            .provenance()
            .declared_origins()
            .into_iter()
            .flatten()
        {
            match origin {
                ProvenanceOrigin::Receiver if callable.receiver().is_none() => {
                    return Err(ProgramIntegrityError::InvalidCallableShape);
                }
                ProvenanceOrigin::Parameter(parameter)
                    if !callable.parameters().contains(parameter) =>
                {
                    return Err(ProgramIntegrityError::OwnerMismatch(
                        DeclarationDomain::Parameter,
                    ));
                }
                ProvenanceOrigin::Receiver | ProvenanceOrigin::Parameter(_) => {}
            }
        }
        let variadic_count = callable
            .parameters()
            .iter()
            .filter(|parameter| {
                matches!(
                    declarations
                        .parameters()
                        .get(**parameter)
                        .copied()
                        .map(crate::Parameter::role),
                    Some(ParameterRole::Ordinary { variadic: true, .. })
                )
            })
            .count();
        let valid_variadic = match callable.kind() {
            CallableKind::Literal(crate::LiteralShape::Sequence) => {
                callable.parameters().len() == 1 && variadic_count == 1
            }
            _ => variadic_count == 0,
        };
        if !valid_variadic {
            return Err(ProgramIntegrityError::InvalidCallableShape);
        }
        if let Some(body) = callable.body() {
            let body = require(
                declarations.bodies().get(body),
                DeclarationDomain::Callable,
                DeclarationDomain::Body,
            )?;
            if body.owner() != BodyOwner::Callable(id) {
                return Err(ProgramIntegrityError::OwnerMismatch(
                    DeclarationDomain::Body,
                ));
            }
        }
    }
    Ok(())
}

fn validate_callable_shape(
    callable: &crate::CallableDeclaration,
) -> Result<(), ProgramIntegrityError> {
    let named = matches!(
        callable.kind(),
        CallableKind::Function
            | CallableKind::Primitive
            | CallableKind::Method
            | CallableKind::ConstructionFunction
    );
    let receiver = matches!(
        callable.kind(),
        CallableKind::Method
            | CallableKind::Coercion
            | CallableKind::Equality
            | CallableKind::Ordering
            | CallableKind::Index
            | CallableKind::Expansion
    );
    let kind_matches_owner = match callable.owner() {
        CallableOwner::Module(_) => {
            matches!(
                callable.kind(),
                CallableKind::Function | CallableKind::Primitive
            )
        }
        CallableOwner::Construction(_) => matches!(
            callable.kind(),
            CallableKind::ConstructionFunction | CallableKind::Literal(_)
        ),
        CallableOwner::Instance(_) => matches!(
            callable.kind(),
            CallableKind::Method
                | CallableKind::Coercion
                | CallableKind::Equality
                | CallableKind::Ordering
                | CallableKind::Index
                | CallableKind::Expansion
        ),
        CallableOwner::Interface(_) | CallableOwner::Conformance(_) => {
            callable.kind() == CallableKind::Method
        }
    };
    let target_gate_allowed = callable.target_gate().is_none()
        || matches!(callable.owner(), CallableOwner::Module(_))
            && matches!(
                callable.kind(),
                CallableKind::Function | CallableKind::Primitive
            );
    let primitive_body = callable.kind() != CallableKind::Primitive || callable.body().is_none();
    if named != callable.name().is_some()
        || receiver != callable.receiver().is_some()
        || !kind_matches_owner
        || !target_gate_allowed
        || !primitive_body
    {
        return Err(ProgramIntegrityError::InvalidCallableShape);
    }
    Ok(())
}

fn validate_callable_owner(
    program: &DeclarationProgram,
    callable: CallableId,
    owner: CallableOwner,
) -> Result<(), ProgramIntegrityError> {
    let declarations = program.declarations();
    let contains = match owner {
        CallableOwner::Module(module) => program.modules().get(module).is_some(),
        CallableOwner::Construction(owner) => declarations
            .constructions()
            .get(owner)
            .is_some_and(|owner| owner.members().contains(&callable)),
        CallableOwner::Instance(owner) => declarations
            .instances()
            .get(owner)
            .is_some_and(|owner| owner.members().contains(&callable)),
        CallableOwner::Interface(owner) => declarations
            .interfaces()
            .get(owner)
            .is_some_and(|owner| owner.methods().contains(&callable)),
        CallableOwner::Conformance(owner) => declarations
            .conformances()
            .get(owner)
            .is_some_and(|owner| owner.methods().contains(&callable)),
    };
    if !contains {
        return Err(ProgramIntegrityError::OwnerMismatch(
            DeclarationDomain::Callable,
        ));
    }
    Ok(())
}

fn validate_constructions_instances_conformances(
    program: &DeclarationProgram,
) -> Result<(), ProgramIntegrityError> {
    let declarations = program.declarations();
    for (id, construction) in declarations.constructions().iter() {
        require_site(
            program,
            construction.site(),
            DeclarationDomain::Construction,
        )?;
        require_type(
            program,
            construction.target(),
            DeclarationDomain::Construction,
        )?;
        unique(
            construction.generic_parameters(),
            DeclarationDomain::Construction,
        )?;
        unique(construction.members(), DeclarationDomain::Construction)?;
        if construction
            .default_member()
            .is_some_and(|default| !construction.members().contains(&default))
        {
            return Err(ProgramIntegrityError::OwnerMismatch(
                DeclarationDomain::Callable,
            ));
        }
        for member in construction.members() {
            let member = require(
                declarations.callables().get(*member),
                DeclarationDomain::Construction,
                DeclarationDomain::Callable,
            )?;
            if member.owner() != CallableOwner::Construction(id) {
                return Err(ProgramIntegrityError::OwnerMismatch(
                    DeclarationDomain::Callable,
                ));
            }
        }
    }
    for (id, instance) in declarations.instances().iter() {
        require_site(program, instance.site(), DeclarationDomain::Instance)?;
        require_type(program, instance.target(), DeclarationDomain::Instance)?;
        unique(instance.generic_parameters(), DeclarationDomain::Instance)?;
        unique(instance.requirements(), DeclarationDomain::Instance)?;
        unique(instance.members(), DeclarationDomain::Instance)?;
        for member in instance.members() {
            let member = require(
                declarations.callables().get(*member),
                DeclarationDomain::Instance,
                DeclarationDomain::Callable,
            )?;
            if member.owner() != CallableOwner::Instance(id) {
                return Err(ProgramIntegrityError::OwnerMismatch(
                    DeclarationDomain::Callable,
                ));
            }
        }
    }
    for (id, conformance) in declarations.conformances().iter() {
        require_site(program, conformance.site(), DeclarationDomain::Conformance)?;
        validate_interface_application(
            program,
            conformance.interface(),
            DeclarationDomain::Conformance,
        )?;
        require_type(
            program,
            conformance.target(),
            DeclarationDomain::Conformance,
        )?;
        unique(
            conformance.generic_parameters(),
            DeclarationDomain::Conformance,
        )?;
        unique(conformance.requirements(), DeclarationDomain::Conformance)?;
        unique(conformance.methods(), DeclarationDomain::Conformance)?;
        validate_associated_bindings(
            program,
            conformance.associated_types(),
            conformance.interface().interface(),
            DeclarationDomain::Conformance,
        )?;
        for method in conformance.methods() {
            let method = require(
                declarations.callables().get(*method),
                DeclarationDomain::Conformance,
                DeclarationDomain::Callable,
            )?;
            if method.owner() != CallableOwner::Conformance(id) {
                return Err(ProgramIntegrityError::OwnerMismatch(
                    DeclarationDomain::Callable,
                ));
            }
        }
    }
    Ok(())
}

fn validate_drops_tests(program: &DeclarationProgram) -> Result<(), ProgramIntegrityError> {
    let declarations = program.declarations();
    for (id, drop) in declarations.drops().iter() {
        require_site(program, drop.site(), DeclarationDomain::Drop)?;
        require_type(program, drop.target(), DeclarationDomain::Drop)?;
        unique(drop.generic_parameters(), DeclarationDomain::Drop)?;
        let body = require(
            declarations.bodies().get(drop.body()),
            DeclarationDomain::Drop,
            DeclarationDomain::Body,
        )?;
        if body.owner() != BodyOwner::Drop(id) {
            return Err(ProgramIntegrityError::OwnerMismatch(
                DeclarationDomain::Body,
            ));
        }
        let receiver = require(
            declarations.parameters().get(drop.receiver()),
            DeclarationDomain::Drop,
            DeclarationDomain::Parameter,
        )?;
        if receiver.owner() != ParameterOwner::Drop(id)
            || receiver.role() != ParameterRole::Receiver(CallableCapability::ReadWrite)
            || receiver.ty() != drop.target()
        {
            return Err(ProgramIntegrityError::OwnerMismatch(
                DeclarationDomain::Parameter,
            ));
        }
    }
    for (id, test) in declarations.tests().iter() {
        require_site(program, test.site(), DeclarationDomain::Test)?;
        require_symbol(program, test.name(), DeclarationDomain::Test)?;
        let body = require(
            declarations.bodies().get(test.body()),
            DeclarationDomain::Test,
            DeclarationDomain::Body,
        )?;
        if body.owner() != BodyOwner::Test(id) {
            return Err(ProgramIntegrityError::OwnerMismatch(
                DeclarationDomain::Body,
            ));
        }
    }
    Ok(())
}

fn validate_generic_parameters(program: &DeclarationProgram) -> Result<(), ProgramIntegrityError> {
    for (id, parameter) in program.declarations().generic_parameters().iter() {
        require_symbol(
            program,
            parameter.name(),
            DeclarationDomain::GenericParameter,
        )?;
        let list = generic_owner_list(program, parameter.owner()).ok_or(
            ProgramIntegrityError::OwnerMismatch(DeclarationDomain::GenericParameter),
        )?;
        if list.get(parameter.position()) != Some(&id) {
            return Err(ProgramIntegrityError::InvalidPosition(
                DeclarationDomain::GenericParameter,
            ));
        }
    }
    Ok(())
}

fn generic_owner_list(
    program: &DeclarationProgram,
    owner: GenericOwner,
) -> Option<&[GenericParameterId]> {
    let declarations = program.declarations();
    match owner {
        GenericOwner::NominalType(owner) => declarations
            .nominal_types()
            .get(owner)
            .map(crate::NominalTypeDeclaration::generic_parameters),
        GenericOwner::TypeAlias(owner) => declarations
            .type_aliases()
            .get(owner)
            .map(crate::TypeAliasDeclaration::generic_parameters),
        GenericOwner::Interface(owner) => declarations
            .interfaces()
            .get(owner)
            .map(crate::InterfaceDeclaration::generic_parameters),
        GenericOwner::Callable(owner) => declarations
            .callables()
            .get(owner)
            .map(crate::CallableDeclaration::generic_parameters),
        GenericOwner::Construction(owner) => declarations
            .constructions()
            .get(owner)
            .map(crate::ConstructionDeclaration::generic_parameters),
        GenericOwner::Instance(owner) => declarations
            .instances()
            .get(owner)
            .map(crate::InstanceDeclaration::generic_parameters),
        GenericOwner::Conformance(owner) => declarations
            .conformances()
            .get(owner)
            .map(crate::ConformanceDeclaration::generic_parameters),
        GenericOwner::Drop(owner) => declarations
            .drops()
            .get(owner)
            .map(crate::DropDeclaration::generic_parameters),
    }
}

fn validate_parameters(program: &DeclarationProgram) -> Result<(), ProgramIntegrityError> {
    let declarations = program.declarations();
    for (id, parameter) in declarations.parameters().iter() {
        require_symbol(program, parameter.name(), DeclarationDomain::Parameter)?;
        require_type(program, parameter.ty(), DeclarationDomain::Parameter)?;
        match parameter.owner() {
            ParameterOwner::Callable(owner) => {
                let callable = require(
                    declarations.callables().get(owner),
                    DeclarationDomain::Parameter,
                    DeclarationDomain::Callable,
                )?;
                match parameter.role() {
                    ParameterRole::Ordinary { position, .. }
                        if callable.parameters().get(position) == Some(&id) => {}
                    ParameterRole::Receiver(capability)
                        if callable.receiver() == Some(id)
                            && valid_receiver_capability(callable.kind(), capability) => {}
                    ParameterRole::Ordinary { .. } | ParameterRole::Receiver(_) => {
                        return Err(ProgramIntegrityError::InvalidPosition(
                            DeclarationDomain::Parameter,
                        ));
                    }
                }
            }
            ParameterOwner::Variant(owner) => {
                let variant = require(
                    declarations.variants().get(owner),
                    DeclarationDomain::Parameter,
                    DeclarationDomain::Variant,
                )?;
                let ParameterRole::Ordinary {
                    position,
                    variadic: false,
                } = parameter.role()
                else {
                    return Err(ProgramIntegrityError::OwnerMismatch(
                        DeclarationDomain::Parameter,
                    ));
                };
                if variant.payload().get(position) != Some(&id) {
                    return Err(ProgramIntegrityError::InvalidPosition(
                        DeclarationDomain::Parameter,
                    ));
                }
            }
            ParameterOwner::Drop(owner) => {
                let drop = require(
                    declarations.drops().get(owner),
                    DeclarationDomain::Parameter,
                    DeclarationDomain::Drop,
                )?;
                if drop.receiver() != id
                    || parameter.role() != ParameterRole::Receiver(CallableCapability::ReadWrite)
                    || parameter.ty() != drop.target()
                {
                    return Err(ProgramIntegrityError::OwnerMismatch(
                        DeclarationDomain::Parameter,
                    ));
                }
            }
        }
    }
    Ok(())
}

fn valid_receiver_capability(kind: CallableKind, capability: CallableCapability) -> bool {
    match kind {
        CallableKind::Coercion | CallableKind::Index => capability != CallableCapability::Owned,
        CallableKind::Equality | CallableKind::Ordering => {
            capability == CallableCapability::Readonly
        }
        CallableKind::Method | CallableKind::Expansion => true,
        CallableKind::Function
        | CallableKind::Primitive
        | CallableKind::ConstructionFunction
        | CallableKind::Literal(_) => false,
    }
}

fn validate_requirements(program: &DeclarationProgram) -> Result<(), ProgramIntegrityError> {
    for (id, requirement) in program.declarations().requirements().iter() {
        if !requirement_owner_contains(program, requirement.owner(), id) {
            return Err(ProgramIntegrityError::OwnerMismatch(
                DeclarationDomain::Requirement,
            ));
        }
        match requirement.kind() {
            RequirementKind::Capability {
                subject,
                capability,
            } => {
                validate_requirement_subject(program, *subject)?;
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

fn validate_requirement_subject(
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

fn requirement_owner_contains(
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

fn validate_bodies(program: &DeclarationProgram) -> Result<(), ProgramIntegrityError> {
    let declarations = program.declarations();
    for (id, body) in declarations.bodies().iter() {
        let reciprocal = match body.owner() {
            BodyOwner::Callable(owner) => declarations
                .callables()
                .get(owner)
                .is_some_and(|owner| owner.body() == Some(id)),
            BodyOwner::Drop(owner) => declarations
                .drops()
                .get(owner)
                .is_some_and(|owner| owner.body() == id),
            BodyOwner::Test(owner) => declarations
                .tests()
                .get(owner)
                .is_some_and(|owner| owner.body() == id),
        };
        if !reciprocal {
            return Err(ProgramIntegrityError::OwnerMismatch(
                DeclarationDomain::Body,
            ));
        }
    }
    Ok(())
}

fn validate_opaque_types(program: &DeclarationProgram) -> Result<(), ProgramIntegrityError> {
    for (_, opaque) in program.declarations().opaque_types().iter() {
        require(
            program.declarations().callables().get(opaque.owner()),
            DeclarationDomain::OpaqueType,
            DeclarationDomain::Callable,
        )?;
        unique(opaque.generic_parameters(), DeclarationDomain::OpaqueType)?;
        for parameter in opaque.generic_parameters() {
            require(
                program.declarations().generic_parameters().get(*parameter),
                DeclarationDomain::OpaqueType,
                DeclarationDomain::GenericParameter,
            )?;
        }
        validate_interface_application(program, opaque.interface(), DeclarationDomain::OpaqueType)?;
        validate_associated_bindings(
            program,
            opaque.associated_types(),
            opaque.interface().interface(),
            DeclarationDomain::OpaqueType,
        )?;
    }
    Ok(())
}

fn validate_associated_bindings(
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

fn validate_interface_application(
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

fn validate_visibility(
    program: &DeclarationProgram,
    declaring_module: ModuleId,
    visibility: Visibility,
    domain: DeclarationDomain,
) -> Result<(), ProgramIntegrityError> {
    let declaring = require(
        program.modules().get(declaring_module),
        domain,
        DeclarationDomain::Module,
    )?;
    match visibility {
        Visibility::Private | Visibility::Public => Ok(()),
        Visibility::Package(package) if package == declaring.package() => Ok(()),
        Visibility::Descendants(boundary) => {
            let boundary = require(
                program.modules().get(boundary),
                domain,
                DeclarationDomain::Module,
            )?;
            if boundary.package() == declaring.package()
                && boundary.path().is_ancestor_of(declaring.path())
            {
                Ok(())
            } else {
                Err(ProgramIntegrityError::InvalidVisibility(domain))
            }
        }
        Visibility::Package(_) => Err(ProgramIntegrityError::InvalidVisibility(domain)),
    }
}

fn require_site(
    program: &DeclarationProgram,
    site: nocter_model::DeclarationSiteId,
    owner: DeclarationDomain,
) -> Result<&crate::DeclarationSite, ProgramIntegrityError> {
    require(
        program.declaration_sites().get(site),
        owner,
        DeclarationDomain::DeclarationSite,
    )
}

fn require_symbol(
    program: &DeclarationProgram,
    symbol: Symbol,
    owner: DeclarationDomain,
) -> Result<(), ProgramIntegrityError> {
    program
        .symbols()
        .spelling(symbol)
        .map(|_| ())
        .ok_or(ProgramIntegrityError::UnknownSymbol(owner))
}

fn require_optional_symbol(
    program: &DeclarationProgram,
    symbol: Option<Symbol>,
    owner: DeclarationDomain,
) -> Result<(), ProgramIntegrityError> {
    symbol.map_or(Ok(()), |symbol| require_symbol(program, symbol, owner))
}

fn require_type(
    program: &DeclarationProgram,
    ty: TypeId,
    owner: DeclarationDomain,
) -> Result<(), ProgramIntegrityError> {
    program
        .types()
        .get(ty)
        .map(|_| ())
        .ok_or(ProgramIntegrityError::UnknownType(owner))
}

fn require<T>(
    value: Option<&T>,
    owner: DeclarationDomain,
    target: DeclarationDomain,
) -> Result<&T, ProgramIntegrityError> {
    value.ok_or(ProgramIntegrityError::UnknownReference { owner, target })
}

fn unique<T: Copy + Eq + Hash>(
    values: &[T],
    owner: DeclarationDomain,
) -> Result<(), ProgramIntegrityError> {
    let mut seen = HashSet::with_capacity(values.len());
    if values.iter().copied().all(|value| seen.insert(value)) {
        Ok(())
    } else {
        Err(ProgramIntegrityError::DuplicateReference(owner))
    }
}
