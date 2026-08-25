use nocter_model::CallableId;

use crate::{
    BodyOwner, CallableKind, CallableOwner, DeclarationProgram, ParameterOwner, ParameterRole,
    ProvenanceAnnotation, ProvenanceOrigin,
};

use super::{
    DeclarationDomain, ProgramIntegrityError, require, require_optional_symbol, require_site,
    require_type, unique,
};

pub(super) fn validate(program: &DeclarationProgram) -> Result<(), ProgramIntegrityError> {
    let declarations = program.declarations();
    for (id, callable) in declarations.callables().iter() {
        require_site(program, callable.site(), DeclarationDomain::Callable)?;
        require_optional_symbol(program, callable.name(), DeclarationDomain::Callable)?;
        require_type(program, callable.result(), DeclarationDomain::Callable)?;
        unique(callable.generic_parameters(), DeclarationDomain::Callable)?;
        unique(callable.parameters(), DeclarationDomain::Callable)?;
        unique(callable.requirements(), DeclarationDomain::Callable)?;
        validate_shape(callable)?;
        if matches!(
            callable.provenance_annotation(),
            ProvenanceAnnotation::Explicit { .. }
        ) && matches!(
            callable.provenance(),
            crate::CallableProvenanceContract::Inferred
        ) {
            return Err(ProgramIntegrityError::InvalidCallableShape);
        }
        validate_owner(program, id, callable.owner())?;
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
        validate_argument_pack_shape(program, callable)?;
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

fn validate_argument_pack_shape(
    program: &DeclarationProgram,
    callable: &crate::CallableDeclaration,
) -> Result<(), ProgramIntegrityError> {
    let pack_positions = callable
        .parameters()
        .iter()
        .enumerate()
        .filter_map(|(position, parameter)| {
            matches!(
                program
                    .declarations()
                    .parameters()
                    .get(*parameter)
                    .copied()
                    .map(crate::Parameter::role),
                Some(ParameterRole::ArgumentPack { .. })
            )
            .then_some(position)
        })
        .collect::<Vec<_>>();
    let valid = match callable.kind() {
        CallableKind::Literal(crate::LiteralShape::Sequence) => {
            callable.parameters().len() == 1 && pack_positions.as_slice() == [0]
        }
        CallableKind::Function | CallableKind::Method | CallableKind::ConstructionFunction => {
            pack_positions.is_empty()
                || pack_positions.as_slice() == [callable.parameters().len() - 1]
        }
        CallableKind::Primitive
        | CallableKind::Literal(crate::LiteralShape::String)
        | CallableKind::Coercion
        | CallableKind::Equality
        | CallableKind::Ordering
        | CallableKind::Index
        | CallableKind::Expansion => pack_positions.is_empty(),
    };
    valid
        .then_some(())
        .ok_or(ProgramIntegrityError::InvalidCallableShape)
}

fn validate_shape(callable: &crate::CallableDeclaration) -> Result<(), ProgramIntegrityError> {
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

fn validate_owner(
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
