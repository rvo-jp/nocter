use nocter_declarations::{
    DeclarationGraph, ExpansionCapability, InterfaceApplication, RequirementKind,
    RequirementSubject, StructuralCapability,
};
use nocter_model::{
    BorrowCapability, CallableContract, RequirementId, TypeId, TypeKind, TypeStore,
};

use crate::type_relations::{SubstitutionError, TypeSubstitution};

/// A declaration requirement normalized to type identities after owner substitution.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum CheckedPredicate {
    Capability {
        subject: TypeId,
        capability: StructuralCapability,
    },
    Copy(TypeId),
    TypeEquality {
        left: TypeId,
        right: TypeId,
    },
    Equality(TypeId),
    Ordering(TypeId),
    Index {
        capability: BorrowCapability,
        container: TypeId,
        index: TypeId,
        result: TypeId,
    },
    Coercion {
        source: TypeId,
        target: TypeId,
    },
    Expansion {
        capability: ExpansionCapability,
        source: TypeId,
        result: TypeId,
    },
}

/// One normalized predicate retaining its declaration identity for diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedRequirement {
    declaration: RequirementId,
    predicate: CheckedPredicate,
}

impl CheckedRequirement {
    pub(super) const fn new(declaration: RequirementId, predicate: CheckedPredicate) -> Self {
        Self {
            declaration,
            predicate,
        }
    }

    #[must_use]
    pub const fn declaration(&self) -> RequirementId {
        self.declaration
    }

    #[must_use]
    pub const fn predicate(&self) -> &CheckedPredicate {
        &self.predicate
    }
}

pub(crate) fn normalize_requirements(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    substitution: &TypeSubstitution,
    requirements: &[RequirementId],
) -> Result<Vec<CheckedRequirement>, SubstitutionError> {
    requirements
        .iter()
        .map(|id| {
            let requirement = graph
                .declarations()
                .requirements()
                .get(*id)
                .ok_or(SubstitutionError::InvalidStore)?;
            normalize_predicate(graph, types, substitution, requirement.kind())
                .map(|predicate| CheckedRequirement::new(*id, predicate))
        })
        .collect()
}

pub(crate) fn substitute_predicate(
    types: &mut TypeStore,
    substitution: &TypeSubstitution,
    predicate: &CheckedPredicate,
) -> Result<CheckedPredicate, SubstitutionError> {
    Ok(match predicate {
        CheckedPredicate::Capability {
            subject,
            capability,
        } => CheckedPredicate::Capability {
            subject: substitution.apply_type(types, *subject)?,
            capability: match capability {
                StructuralCapability::Interface(application) => {
                    StructuralCapability::Interface(InterfaceApplication::new(
                        application.interface(),
                        application
                            .arguments()
                            .iter()
                            .map(|argument| substitution.apply_type(types, *argument))
                            .collect::<Result<Vec<_>, _>>()?,
                    ))
                }
                StructuralCapability::Callable(contract) => StructuralCapability::Callable(
                    substitute_callable(types, substitution, contract)?,
                ),
            },
        },
        CheckedPredicate::Copy(ty) => CheckedPredicate::Copy(substitution.apply_type(types, *ty)?),
        CheckedPredicate::TypeEquality { left, right } => CheckedPredicate::TypeEquality {
            left: substitution.apply_type(types, *left)?,
            right: substitution.apply_type(types, *right)?,
        },
        CheckedPredicate::Equality(ty) => {
            CheckedPredicate::Equality(substitution.apply_type(types, *ty)?)
        }
        CheckedPredicate::Ordering(ty) => {
            CheckedPredicate::Ordering(substitution.apply_type(types, *ty)?)
        }
        CheckedPredicate::Index {
            capability,
            container,
            index,
            result,
        } => CheckedPredicate::Index {
            capability: *capability,
            container: substitution.apply_type(types, *container)?,
            index: substitution.apply_type(types, *index)?,
            result: substitution.apply_type(types, *result)?,
        },
        CheckedPredicate::Coercion { source, target } => CheckedPredicate::Coercion {
            source: substitution.apply_type(types, *source)?,
            target: substitution.apply_type(types, *target)?,
        },
        CheckedPredicate::Expansion {
            capability,
            source,
            result,
        } => CheckedPredicate::Expansion {
            capability: *capability,
            source: substitution.apply_type(types, *source)?,
            result: substitution.apply_type(types, *result)?,
        },
    })
}

fn normalize_predicate(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    substitution: &TypeSubstitution,
    requirement: &RequirementKind,
) -> Result<CheckedPredicate, SubstitutionError> {
    Ok(match requirement {
        RequirementKind::Capability {
            subject,
            capability,
        } => CheckedPredicate::Capability {
            subject: subject_type(graph, types, substitution, *subject)?,
            capability: match capability {
                StructuralCapability::Interface(application) => {
                    StructuralCapability::Interface(InterfaceApplication::new(
                        application.interface(),
                        application
                            .arguments()
                            .iter()
                            .map(|argument| substitution.apply_type(types, *argument))
                            .collect::<Result<Vec<_>, _>>()?,
                    ))
                }
                StructuralCapability::Callable(contract) => StructuralCapability::Callable(
                    substitute_callable(types, substitution, contract)?,
                ),
            },
        },
        RequirementKind::Copy(parameter) => {
            CheckedPredicate::Copy(generic_type(types, substitution, *parameter)?)
        }
        RequirementKind::TypeEquality { left, right } => CheckedPredicate::TypeEquality {
            left: substitution.apply_type(types, *left)?,
            right: substitution.apply_type(types, *right)?,
        },
        RequirementKind::Equality { operand } => {
            CheckedPredicate::Equality(generic_type(types, substitution, *operand)?)
        }
        RequirementKind::Ordering { operand } => {
            CheckedPredicate::Ordering(generic_type(types, substitution, *operand)?)
        }
        RequirementKind::Index {
            capability,
            container,
            index,
            result,
        } => CheckedPredicate::Index {
            capability: *capability,
            container: generic_type(types, substitution, *container)?,
            index: substitution.apply_type(types, *index)?,
            result: substitution.apply_type(types, *result)?,
        },
        RequirementKind::Coercion { source, target } => CheckedPredicate::Coercion {
            source: substitution.apply_type(types, *source)?,
            target: substitution.apply_type(types, *target)?,
        },
        RequirementKind::Expansion {
            capability,
            source,
            result,
        } => CheckedPredicate::Expansion {
            capability: *capability,
            source: generic_type(types, substitution, *source)?,
            result: substitution.apply_type(types, *result)?,
        },
        RequirementKind::BinderRefinement {
            parameter,
            replacement,
        } => CheckedPredicate::TypeEquality {
            left: generic_type(types, substitution, *parameter)?,
            right: substitution.apply_type(types, *replacement)?,
        },
    })
}

fn generic_type(
    types: &mut TypeStore,
    substitution: &TypeSubstitution,
    parameter: nocter_model::GenericParameterId,
) -> Result<TypeId, SubstitutionError> {
    let ty = types
        .intern(TypeKind::GenericParameter(parameter))
        .map_err(|_| SubstitutionError::InvalidStore)?;
    substitution.apply_type(types, ty)
}

fn subject_type(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    substitution: &TypeSubstitution,
    subject: RequirementSubject,
) -> Result<TypeId, SubstitutionError> {
    let ty = match subject {
        RequirementSubject::GenericParameter(parameter) => types
            .intern(TypeKind::GenericParameter(parameter))
            .map_err(|_| SubstitutionError::InvalidStore)?,
        RequirementSubject::AssociatedType(associated) => {
            let declaration = graph
                .declarations()
                .associated_types()
                .get(associated)
                .ok_or(SubstitutionError::InvalidStore)?;
            let base = types
                .intern(TypeKind::InterfaceSelf(declaration.interface()))
                .map_err(|_| SubstitutionError::InvalidStore)?;
            types
                .intern(TypeKind::AssociatedProjection { base, associated })
                .map_err(|_| SubstitutionError::InvalidStore)?
        }
    };
    substitution.apply_type(types, ty)
}

fn substitute_callable(
    types: &mut TypeStore,
    substitution: &TypeSubstitution,
    contract: &CallableContract,
) -> Result<CallableContract, SubstitutionError> {
    CallableContract::new(
        contract.capability(),
        contract
            .parameters()
            .iter()
            .map(|parameter| substitution.apply_type(types, *parameter))
            .collect::<Result<Vec<_>, _>>()?,
        substitution.apply_type(types, contract.result())?,
        contract.provenance().clone(),
    )
    .map_err(|_| SubstitutionError::InvalidStore)
}
