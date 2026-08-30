use nocter_declarations::{
    AssociatedTypeBinding, DeclarationGraph, ExpansionCapability, InterfaceApplication,
    RequirementKind, RequirementSubject,
};
use nocter_model::{BorrowCapability, CallableContract, RequirementId, TypeId, TypeKind};

use crate::type_relations::{SubstitutionError, TypeSubstitution};

/// A declaration requirement normalized to type identities after owner substitution.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum CheckedPredicate {
    Interface {
        subject: TypeId,
        application: InterfaceApplication,
        associated_types: Box<[AssociatedTypeBinding]>,
    },
    Callable {
        subject: TypeId,
        contract: CallableContract,
    },
    Copy(TypeId),
    BinderRefinement {
        binder: TypeId,
        replacement: TypeId,
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

/// One authored derivation of a normalized declaration predicate.
///
/// `root` is the requirement written on the declaration being checked. `origin` is the exact
/// requirement declaration that contributed the predicate; the two differ for a transitive
/// interface prerequisite.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct RequirementDerivation {
    root: RequirementId,
    origin: RequirementId,
}

impl RequirementDerivation {
    const fn new(root: RequirementId, origin: RequirementId) -> Self {
        Self { root, origin }
    }

    #[must_use]
    pub const fn root(self) -> RequirementId {
        self.root
    }

    #[must_use]
    pub const fn origin(self) -> RequirementId {
        self.origin
    }
}

/// One normalized declaration predicate used while proving declarations.
///
/// Predicate identity is the semantic fact. `derivations` preserves every authored route that
/// establishes that fact without making the first route authoritative. Runtime/body evidence is
/// deliberately represented by `BodyRequirement` instead of being optional here.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedRequirement {
    derivations: Vec<RequirementDerivation>,
    predicate: CheckedPredicate,
}

impl CheckedRequirement {
    fn new(derivation: RequirementDerivation, predicate: CheckedPredicate) -> Self {
        Self {
            derivations: vec![derivation],
            predicate,
        }
    }

    #[must_use]
    pub fn derivations(&self) -> &[RequirementDerivation] {
        &self.derivations
    }

    fn add_derivation(&mut self, derivation: RequirementDerivation) {
        if !self.derivations.contains(&derivation) {
            self.derivations.push(derivation);
        }
    }

    #[must_use]
    pub const fn predicate(&self) -> &CheckedPredicate {
        &self.predicate
    }
}

pub(crate) trait RequirementPredicate {
    fn predicate(&self) -> &CheckedPredicate;
}

impl RequirementPredicate for CheckedRequirement {
    fn predicate(&self) -> &CheckedPredicate {
        self.predicate()
    }
}

pub(crate) fn normalize_requirements(
    graph: &DeclarationGraph,
    types: &mut nocter_model::TypeTransaction,
    substitution: &TypeSubstitution,
    requirements: &[RequirementId],
) -> Result<Vec<CheckedRequirement>, SubstitutionError> {
    let mut normalized: Vec<CheckedRequirement> = Vec::new();
    let mut indexes: HashMap<CheckedPredicate, usize> = HashMap::new();
    for id in requirements {
        let requirement = graph
            .declarations()
            .requirements()
            .get(*id)
            .ok_or(SubstitutionError::InvalidStore)?;
        let predicate = normalize_predicate(graph, types, substitution, requirement.kind())?;
        let mut pending = VecDeque::from([(predicate, RequirementDerivation::new(*id, *id))]);
        let mut expanded = HashSet::new();
        while let Some((predicate, derivation)) = pending.pop_front() {
            if let Some(index) = indexes.get(&predicate).copied() {
                normalized[index].add_derivation(derivation);
            } else {
                indexes.insert(predicate.clone(), normalized.len());
                normalized.push(CheckedRequirement::new(derivation, predicate.clone()));
            }
            if !expanded.insert(predicate.clone()) {
                continue;
            }
            if let CheckedPredicate::Interface { application, .. } = &predicate {
                let capability = graph
                    .interface_capabilities()
                    .get(application.interface())
                    .ok_or(SubstitutionError::InvalidStore)?;
                for prerequisite in capability.direct_prerequisites() {
                    let predicate =
                        specialize_prerequisite(graph, types, &predicate, *prerequisite)?;
                    pending.push_back((predicate, RequirementDerivation::new(*id, *prerequisite)));
                }
            }
        }
    }
    Ok(normalized)
}

fn specialize_prerequisite(
    graph: &DeclarationGraph,
    types: &mut nocter_model::TypeTransaction,
    context: &CheckedPredicate,
    prerequisite: RequirementId,
) -> Result<CheckedPredicate, SubstitutionError> {
    let CheckedPredicate::Interface {
        subject,
        application,
        associated_types,
    } = context
    else {
        return Err(SubstitutionError::InvalidStore);
    };
    let interface_id = application.interface();
    let interface = graph
        .declarations()
        .interfaces()
        .get(interface_id)
        .ok_or(SubstitutionError::InvalidStore)?;
    let mut substitution = TypeSubstitution::default();
    substitution.set_interface_self(interface_id, *subject);
    for (parameter, argument) in interface
        .generic_parameters()
        .iter()
        .zip(application.arguments())
    {
        substitution.bind_generic(*parameter, *argument);
    }
    for binding in associated_types {
        substitution.bind_associated(binding.declaration(), binding.ty());
    }
    let requirement = graph
        .declarations()
        .requirements()
        .get(prerequisite)
        .ok_or(SubstitutionError::InvalidStore)?;
    Ok(inherit_associated_bindings(
        graph,
        normalize_predicate(graph, types, &substitution, requirement.kind())?,
        associated_types,
    ))
}

fn inherit_associated_bindings(
    graph: &DeclarationGraph,
    predicate: CheckedPredicate,
    inherited: &[AssociatedTypeBinding],
) -> CheckedPredicate {
    let CheckedPredicate::Interface {
        subject,
        application,
        associated_types,
    } = predicate
    else {
        return predicate;
    };
    let mut bindings = associated_types.into_vec();
    for binding in inherited {
        let Some(declaration) = graph
            .declarations()
            .associated_types()
            .get(binding.declaration())
        else {
            continue;
        };
        if graph
            .interface_capabilities()
            .entails(application.interface(), declaration.interface())
            && !bindings
                .iter()
                .any(|candidate| candidate.declaration() == binding.declaration())
        {
            bindings.push(*binding);
        }
    }
    bindings.sort_unstable_by_key(|binding| binding.declaration());
    CheckedPredicate::Interface {
        subject,
        application,
        associated_types: bindings.into_boxed_slice(),
    }
}

pub(crate) fn substitute_predicate(
    types: &mut nocter_model::TypeTransaction,
    substitution: &TypeSubstitution,
    predicate: &CheckedPredicate,
) -> Result<CheckedPredicate, SubstitutionError> {
    Ok(match predicate {
        CheckedPredicate::Interface {
            subject,
            application,
            associated_types,
        } => CheckedPredicate::Interface {
            subject: substitution.apply_type(types, *subject)?,
            application: InterfaceApplication::new(
                application.interface(),
                application
                    .arguments()
                    .iter()
                    .map(|argument| substitution.apply_type(types, *argument))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            associated_types: associated_types
                .iter()
                .map(|binding| {
                    Ok(AssociatedTypeBinding::new(
                        binding.declaration(),
                        substitution.apply_type(types, binding.ty())?,
                    ))
                })
                .collect::<Result<Vec<_>, SubstitutionError>>()?
                .into_boxed_slice(),
        },
        CheckedPredicate::Callable { subject, contract } => CheckedPredicate::Callable {
            subject: substitution.apply_type(types, *subject)?,
            contract: substitute_callable(types, substitution, contract)?,
        },
        CheckedPredicate::Copy(ty) => CheckedPredicate::Copy(substitution.apply_type(types, *ty)?),
        CheckedPredicate::BinderRefinement {
            binder,
            replacement,
        } => CheckedPredicate::BinderRefinement {
            binder: substitution.apply_type(types, *binder)?,
            replacement: substitution.apply_type(types, *replacement)?,
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
    types: &mut nocter_model::TypeTransaction,
    substitution: &TypeSubstitution,
    requirement: &RequirementKind,
) -> Result<CheckedPredicate, SubstitutionError> {
    Ok(match requirement {
        RequirementKind::Interface {
            subject,
            application,
            associated_types,
        } => CheckedPredicate::Interface {
            subject: subject_type(graph, types, substitution, *subject)?,
            application: InterfaceApplication::new(
                application.interface(),
                application
                    .arguments()
                    .iter()
                    .map(|argument| substitution.apply_type(types, *argument))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            associated_types: associated_types
                .iter()
                .map(|binding| {
                    Ok(AssociatedTypeBinding::new(
                        binding.declaration(),
                        substitution.apply_type(types, binding.ty())?,
                    ))
                })
                .collect::<Result<Vec<_>, SubstitutionError>>()?
                .into_boxed_slice(),
        },
        RequirementKind::Callable { subject, contract } => CheckedPredicate::Callable {
            subject: generic_type(types, substitution, *subject)?,
            contract: substitute_callable(types, substitution, contract)?,
        },
        RequirementKind::Copy(parameter) => {
            CheckedPredicate::Copy(generic_type(types, substitution, *parameter)?)
        }
        RequirementKind::Equality { operand } => {
            CheckedPredicate::Equality(substitution.apply_type(types, *operand)?)
        }
        RequirementKind::Ordering { operand } => {
            CheckedPredicate::Ordering(substitution.apply_type(types, *operand)?)
        }
        RequirementKind::Index {
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
            source: substitution.apply_type(types, *source)?,
            result: substitution.apply_type(types, *result)?,
        },
        RequirementKind::BinderRefinement {
            parameter,
            replacement,
        } => CheckedPredicate::BinderRefinement {
            binder: generic_type(types, substitution, *parameter)?,
            replacement: substitution.apply_type(types, *replacement)?,
        },
    })
}

fn generic_type(
    types: &mut nocter_model::TypeTransaction,
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
    types: &mut nocter_model::TypeTransaction,
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
        RequirementSubject::InterfaceSelf(interface) => types
            .intern(TypeKind::InterfaceSelf(interface))
            .map_err(|_| SubstitutionError::InvalidStore)?,
    };
    substitution.apply_type(types, ty)
}

fn substitute_callable(
    types: &mut nocter_model::TypeTransaction,
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
        contract
            .pack()
            .map(|pack| pack.try_map(|ty| substitution.apply_type(types, ty)))
            .transpose()?,
        substitution.apply_type(types, contract.result())?,
        contract.provenance().clone(),
    )
    .map_err(|_| SubstitutionError::InvalidStore)
}
use std::collections::{HashMap, HashSet, VecDeque};
