use std::fmt;

use nocter_declarations::{
    DeclarationGraph, InterfaceApplication, ParameterRole, RequirementKind, StructuralCapability,
};
use nocter_diagnostics::SourceDiagnostic;
use nocter_model::{TypeId, TypeStore};
use nocter_source_index::{SemanticEntity, SourceIndex, SourceOrigin, SourceRole};

use super::shape::{TypePosition, TypeValidityFailure, validate_type};

#[derive(Debug)]
pub enum DeclarationTypeValidityError {
    Rule(SourceDiagnostic),
    Internal(TypeValidityInternalError),
}

impl DeclarationTypeValidityError {
    #[must_use]
    pub const fn source_diagnostic(&self) -> Option<&SourceDiagnostic> {
        match self {
            Self::Rule(diagnostic) => Some(diagnostic),
            Self::Internal(_) => None,
        }
    }
}

impl fmt::Display for DeclarationTypeValidityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rule(diagnostic) => {
                write!(formatter, "{}: {}", diagnostic.code(), diagnostic.message())
            }
            Self::Internal(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for DeclarationTypeValidityError {}

impl From<TypeValidityInternalError> for DeclarationTypeValidityError {
    fn from(error: TypeValidityInternalError) -> Self {
        Self::Internal(error)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeValidityInternalError {
    UnknownType(TypeId),
    MissingSource(SemanticEntity),
}

impl fmt::Display for TypeValidityInternalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownType(ty) => write!(formatter, "unknown type {ty:?} during validation"),
            Self::MissingSource(entity) => write!(formatter, "missing source for {entity:?}"),
        }
    }
}

impl std::error::Error for TypeValidityInternalError {}

/// Validates every normalized declaration-owned type position.
///
/// Type aliases receive an alias-root position, which permits aliases to `void`, `never`, `str`,
/// and `[T]` without allowing those types when the alias is consumed in a data position.
///
/// # Errors
///
/// Returns one source-backed type rule or an internal error for an inconsistent type/source graph.
pub fn validate_declaration_types(
    graph: &DeclarationGraph,
    types: &TypeStore,
    source_index: &SourceIndex,
) -> Result<(), DeclarationTypeValidityError> {
    let declarations = graph.declarations();
    validate_value_positions(graph, types, source_index)?;
    for (id, construction) in declarations.constructions().iter() {
        validate_position(
            types,
            source_index,
            construction.target(),
            TypePosition::TypeOperand,
            SemanticEntity::Construction(id),
        )?;
    }
    for (id, instance) in declarations.instances().iter() {
        validate_position(
            types,
            source_index,
            instance.target(),
            TypePosition::TypeOperand,
            SemanticEntity::Instance(id),
        )?;
    }
    for (id, conformance) in declarations.conformances().iter() {
        let entity = SemanticEntity::Conformance(id);
        validate_position(
            types,
            source_index,
            conformance.target(),
            TypePosition::TypeOperand,
            entity,
        )?;
        validate_interface(types, source_index, conformance.interface(), entity)?;
        for binding in conformance.associated_types() {
            validate_position(
                types,
                source_index,
                binding.ty(),
                TypePosition::Data,
                entity,
            )?;
        }
    }
    for (id, drop) in declarations.drops().iter() {
        validate_position(
            types,
            source_index,
            drop.target(),
            TypePosition::TypeOperand,
            SemanticEntity::Drop(id),
        )?;
    }
    for (id, opaque) in declarations.opaque_types().iter() {
        let entity = SemanticEntity::OpaqueType(id);
        validate_interface(types, source_index, opaque.interface(), entity)?;
        for binding in opaque.associated_types() {
            validate_position(
                types,
                source_index,
                binding.ty(),
                TypePosition::Data,
                entity,
            )?;
        }
    }
    for (id, requirement) in declarations.requirements().iter() {
        validate_requirement(
            types,
            source_index,
            requirement.kind(),
            SemanticEntity::Requirement(id),
        )?;
    }
    Ok(())
}

fn validate_value_positions(
    graph: &DeclarationGraph,
    types: &TypeStore,
    source_index: &SourceIndex,
) -> Result<(), DeclarationTypeValidityError> {
    let declarations = graph.declarations();
    for (id, alias) in declarations.type_aliases().iter() {
        validate_position(
            types,
            source_index,
            alias.target(),
            TypePosition::TypeOperand,
            SemanticEntity::TypeAlias(id),
        )?;
    }
    for (id, field) in declarations.fields().iter() {
        validate_position(
            types,
            source_index,
            field.ty(),
            TypePosition::Data,
            SemanticEntity::Field(id),
        )?;
    }
    for (id, parameter) in declarations.parameters().iter() {
        let position = match parameter.role() {
            ParameterRole::Ordinary { .. } | ParameterRole::ArgumentPack { .. } => {
                TypePosition::Data
            }
            ParameterRole::Receiver(_) => TypePosition::TypeOperand,
        };
        validate_position(
            types,
            source_index,
            parameter.ty(),
            position,
            SemanticEntity::Parameter(id),
        )?;
    }
    for (id, callable) in declarations.callables().iter() {
        validate_position(
            types,
            source_index,
            callable.result(),
            TypePosition::CallableResult,
            SemanticEntity::Callable(id),
        )?;
    }
    Ok(())
}

fn validate_requirement(
    types: &TypeStore,
    source_index: &SourceIndex,
    requirement: &RequirementKind,
    entity: SemanticEntity,
) -> Result<(), DeclarationTypeValidityError> {
    match requirement {
        RequirementKind::Capability { capability, .. } => match capability {
            StructuralCapability::Interface(application) => {
                validate_interface(types, source_index, application, entity)
            }
            StructuralCapability::Callable(contract) => {
                for parameter in contract.parameters() {
                    validate_position(types, source_index, *parameter, TypePosition::Data, entity)?;
                }
                if let Some(pack) = contract.pack() {
                    validate_position(types, source_index, pack, TypePosition::Data, entity)?;
                }
                validate_position(
                    types,
                    source_index,
                    contract.result(),
                    TypePosition::CallableResult,
                    entity,
                )
            }
        },
        RequirementKind::TypeEquality { left, right } => {
            validate_position(types, source_index, *left, TypePosition::Data, entity)?;
            validate_position(types, source_index, *right, TypePosition::Data, entity)
        }
        RequirementKind::Index { index, result, .. } => {
            validate_position(types, source_index, *index, TypePosition::Data, entity)?;
            validate_position(types, source_index, *result, TypePosition::Data, entity)
        }
        RequirementKind::Coercion { source, target } => {
            validate_position(types, source_index, *source, TypePosition::Data, entity)?;
            validate_position(types, source_index, *target, TypePosition::Data, entity)
        }
        RequirementKind::Expansion { result, .. } => {
            validate_position(types, source_index, *result, TypePosition::Data, entity)
        }
        RequirementKind::BinderRefinement { replacement, .. } => validate_position(
            types,
            source_index,
            *replacement,
            TypePosition::Data,
            entity,
        ),
        RequirementKind::Copy(_)
        | RequirementKind::Equality { .. }
        | RequirementKind::Ordering { .. } => Ok(()),
    }
}

fn validate_interface(
    types: &TypeStore,
    source_index: &SourceIndex,
    application: &InterfaceApplication,
    entity: SemanticEntity,
) -> Result<(), DeclarationTypeValidityError> {
    for argument in application.arguments() {
        validate_position(types, source_index, *argument, TypePosition::Data, entity)?;
    }
    Ok(())
}

fn validate_position(
    types: &TypeStore,
    source_index: &SourceIndex,
    ty: TypeId,
    position: TypePosition,
    entity: SemanticEntity,
) -> Result<(), DeclarationTypeValidityError> {
    match validate_type(types, ty, position) {
        Ok(()) => Ok(()),
        Err(TypeValidityFailure::Rule(violation)) => {
            let origin = source_origin(source_index, entity)
                .ok_or(TypeValidityInternalError::MissingSource(entity))?;
            Err(DeclarationTypeValidityError::Rule(
                violation.rule().diagnostic(origin),
            ))
        }
        Err(TypeValidityFailure::UnknownType(unknown)) => Err(
            DeclarationTypeValidityError::Internal(TypeValidityInternalError::UnknownType(unknown)),
        ),
    }
}

fn source_origin(source_index: &SourceIndex, entity: SemanticEntity) -> Option<SourceOrigin> {
    source_index
        .bindings_for(entity)
        .iter()
        .find(|binding| {
            matches!(
                binding.role(),
                SourceRole::Declaration | SourceRole::Implementation
            )
        })
        .map(|binding| binding.origin())
}
