use std::fmt;

use nocter_declarations::{CallableKind, DeclarationGraph, ParameterRole};
use nocter_model::{BorrowCapability, CallableCapability, ModuleId, TypeId, TypeKind, TypeStore};

use super::InstanceOperationTable;
use crate::conformance::{normalize_requirements, proves_predicate, substitute_predicate};
use crate::type_relations::{
    SubstitutionError, TypeSubstitution, is_concrete_type, match_type_pattern,
};
use crate::{
    CheckedPredicate, CheckedRequirement, ConformanceTable, Copyability, CopyabilityError,
    CopyabilityTable, GenericArgument, GenericArguments, StaticDispatch, StaticSelection,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct IndexOperationCandidate {
    index: TypeId,
    result: TypeId,
    operation: Option<StaticSelection>,
    receiver_coercion: Option<StaticSelection>,
}

impl IndexOperationCandidate {
    pub(crate) const fn index(&self) -> TypeId {
        self.index
    }

    pub(crate) const fn result(&self) -> TypeId {
        self.result
    }

    pub(crate) const fn operation(&self) -> Option<&StaticSelection> {
        self.operation.as_ref()
    }

    pub(crate) const fn receiver_coercion(&self) -> Option<&StaticSelection> {
        self.receiver_coercion.as_ref()
    }

    pub(crate) const fn is_direct(&self) -> bool {
        self.receiver_coercion.is_none()
    }
}

struct ApplicableInstance {
    instance: nocter_model::InstanceId,
    substitution: TypeSubstitution,
    generic_arguments: GenericArguments,
}

struct CoercionCandidate {
    target: TypeId,
    selection: StaticSelection,
}

#[derive(Debug)]
pub enum InstanceSelectionError {
    MissingInstance(nocter_model::InstanceId),
    MissingCallable(nocter_model::CallableId),
    MissingParameter(nocter_model::ParameterId),
    MissingSite(nocter_model::DeclarationSiteId),
    InvalidIndexSignature(nocter_model::CallableId),
    InvalidCoercionSignature(nocter_model::CallableId),
    InvalidStructuralIndex(nocter_model::RequirementId),
    IncompleteGeneric(nocter_model::GenericParameterId),
    DuplicateGeneric(nocter_model::GenericParameterId),
    Substitution(SubstitutionError),
    Copyability(CopyabilityError),
}

impl fmt::Display for InstanceSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingInstance(instance) => write!(formatter, "missing instance {instance:?}"),
            Self::MissingCallable(callable) => write!(formatter, "missing callable {callable:?}"),
            Self::MissingParameter(parameter) => {
                write!(formatter, "missing parameter {parameter:?}")
            }
            Self::MissingSite(site) => write!(formatter, "missing declaration site {site:?}"),
            Self::InvalidIndexSignature(callable) => {
                write!(formatter, "invalid index operation signature {callable:?}")
            }
            Self::InvalidCoercionSignature(callable) => {
                write!(formatter, "invalid coercion signature {callable:?}")
            }
            Self::InvalidStructuralIndex(requirement) => {
                write!(
                    formatter,
                    "invalid structural index requirement {requirement:?}"
                )
            }
            Self::IncompleteGeneric(parameter) => {
                write!(formatter, "operation selection did not bind {parameter:?}")
            }
            Self::DuplicateGeneric(parameter) => {
                write!(formatter, "operation selection bound {parameter:?} twice")
            }
            Self::Substitution(error) => error.fmt(formatter),
            Self::Copyability(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for InstanceSelectionError {}

impl From<SubstitutionError> for InstanceSelectionError {
    fn from(error: SubstitutionError) -> Self {
        Self::Substitution(error)
    }
}

/// Selects every visible, requirement-satisfied direct index operation on one receiver type.
///
/// Candidate order has no semantic meaning. The caller checks the index expression once and must
/// reject a retained set other than exactly one.
#[allow(clippy::too_many_arguments)]
pub(crate) fn select_index_operations(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    conformances: &ConformanceTable,
    copyabilities: &mut CopyabilityTable,
    table: &InstanceOperationTable,
    assumptions: &[CheckedRequirement],
    from: ModuleId,
    target: TypeId,
    capability: BorrowCapability,
) -> Result<Vec<IndexOperationCandidate>, InstanceSelectionError> {
    let mut selected = structural_index_operations(types, assumptions, target, capability)?;
    selected.extend(select_instance_index_operations(
        graph,
        types,
        conformances,
        copyabilities,
        table,
        assumptions,
        from,
        target,
        capability,
    )?);
    Ok(selected)
}

fn structural_index_operations(
    types: &TypeStore,
    assumptions: &[CheckedRequirement],
    target: TypeId,
    capability: BorrowCapability,
) -> Result<Vec<IndexOperationCandidate>, InstanceSelectionError> {
    let mut selected = Vec::new();
    for assumption in assumptions {
        let CheckedPredicate::Index {
            capability: required_capability,
            container,
            index,
            result,
        } = assumption.predicate()
        else {
            continue;
        };
        if *required_capability != capability || *container != target {
            continue;
        }
        let (result_capability, referent) = borrow_result(types, *result).ok_or(
            InstanceSelectionError::InvalidStructuralIndex(assumption.declaration()),
        )?;
        if result_capability != capability {
            return Err(InstanceSelectionError::InvalidStructuralIndex(
                assumption.declaration(),
            ));
        }
        selected.push(IndexOperationCandidate {
            index: *index,
            result: referent,
            operation: Some(StaticSelection::new(
                StaticDispatch::StructuralRequirement(assumption.declaration()),
                GenericArguments::default(),
            )),
            receiver_coercion: None,
        });
    }
    Ok(selected)
}

#[allow(clippy::too_many_arguments)]
fn select_instance_index_operations(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    conformances: &ConformanceTable,
    copyabilities: &mut CopyabilityTable,
    table: &InstanceOperationTable,
    assumptions: &[CheckedRequirement],
    from: ModuleId,
    target: TypeId,
    capability: BorrowCapability,
) -> Result<Vec<IndexOperationCandidate>, InstanceSelectionError> {
    let mut selected = Vec::new();
    for applicable in applicable_instances(
        graph,
        types,
        conformances,
        copyabilities,
        table,
        assumptions,
        target,
    )? {
        let entry = table
            .entries()
            .get(applicable.instance)
            .ok_or(InstanceSelectionError::MissingInstance(applicable.instance))?;
        for member in entry.members() {
            let callable = graph
                .declarations()
                .callables()
                .get(*member)
                .ok_or(InstanceSelectionError::MissingCallable(*member))?;
            if callable.kind() != CallableKind::Index
                || !visible_callable(graph, from, callable.site())?
            {
                continue;
            }
            let receiver = callable
                .receiver()
                .and_then(|receiver| graph.declarations().parameters().get(receiver))
                .ok_or(InstanceSelectionError::InvalidIndexSignature(*member))?;
            if receiver.role() != ParameterRole::Receiver(callable_capability(capability))
                || callable.parameters().len() != 1
                || !callable.generic_parameters().is_empty()
            {
                continue;
            }
            let parameter = graph
                .declarations()
                .parameters()
                .get(callable.parameters()[0])
                .ok_or(InstanceSelectionError::MissingParameter(
                    callable.parameters()[0],
                ))?;
            let index = applicable.substitution.apply_type(types, parameter.ty())?;
            let result = applicable
                .substitution
                .apply_type(types, callable.result())?;
            let (result_capability, referent) = borrow_result(types, result)
                .ok_or(InstanceSelectionError::InvalidIndexSignature(*member))?;
            if result_capability != capability {
                return Err(InstanceSelectionError::InvalidIndexSignature(*member));
            }
            let callable_requirements = normalize_requirements(
                graph,
                types,
                &applicable.substitution,
                callable.requirements(),
            )?;
            if !requirements_hold(
                graph,
                types,
                conformances,
                copyabilities,
                assumptions,
                &callable_requirements,
                &TypeSubstitution::default(),
            )? {
                continue;
            }
            selected.push(IndexOperationCandidate {
                index,
                result: referent,
                operation: Some(StaticSelection::new(
                    StaticDispatch::Direct(*member),
                    applicable.generic_arguments.clone(),
                )),
                receiver_coercion: None,
            });
        }
    }
    Ok(selected)
}

/// Selects one receiver coercion followed by one built-in or source-defined index operation.
#[allow(clippy::too_many_arguments)]
pub(crate) fn select_coerced_index_operations(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    conformances: &ConformanceTable,
    copyabilities: &mut CopyabilityTable,
    table: &InstanceOperationTable,
    assumptions: &[CheckedRequirement],
    from: ModuleId,
    source: TypeId,
    capability: BorrowCapability,
) -> Result<Vec<IndexOperationCandidate>, InstanceSelectionError> {
    let coercions = select_coercions(
        graph,
        types,
        conformances,
        copyabilities,
        table,
        assumptions,
        from,
        source,
        capability,
    )?;
    let mut selected = Vec::new();
    for coercion in coercions {
        if let Some(result) = builtin_index_result(types, coercion.target, capability) {
            selected.push(IndexOperationCandidate {
                index: types.builtin(nocter_model::BuiltinType::Usize),
                result,
                operation: None,
                receiver_coercion: Some(coercion.selection),
            });
            continue;
        }
        let direct = select_index_operations(
            graph,
            types,
            conformances,
            copyabilities,
            table,
            assumptions,
            from,
            coercion.target,
            capability,
        )?;
        for mut candidate in direct {
            candidate.receiver_coercion = Some(coercion.selection.clone());
            selected.push(candidate);
        }
    }
    Ok(selected)
}

#[allow(clippy::too_many_arguments)]
fn select_coercions(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    conformances: &ConformanceTable,
    copyabilities: &mut CopyabilityTable,
    table: &InstanceOperationTable,
    assumptions: &[CheckedRequirement],
    from: ModuleId,
    source: TypeId,
    capability: BorrowCapability,
) -> Result<Vec<CoercionCandidate>, InstanceSelectionError> {
    let mut selected = Vec::new();
    for applicable in applicable_instances(
        graph,
        types,
        conformances,
        copyabilities,
        table,
        assumptions,
        source,
    )? {
        let entry = table
            .entries()
            .get(applicable.instance)
            .ok_or(InstanceSelectionError::MissingInstance(applicable.instance))?;
        for member in entry.members() {
            let callable = graph
                .declarations()
                .callables()
                .get(*member)
                .ok_or(InstanceSelectionError::MissingCallable(*member))?;
            if callable.kind() != CallableKind::Coercion
                || !visible_callable(graph, from, callable.site())?
            {
                continue;
            }
            let receiver = callable
                .receiver()
                .and_then(|receiver| graph.declarations().parameters().get(receiver))
                .ok_or(InstanceSelectionError::InvalidCoercionSignature(*member))?;
            if receiver.role() != ParameterRole::Receiver(callable_capability(capability))
                || !callable.parameters().is_empty()
                || !callable.generic_parameters().is_empty()
            {
                continue;
            }
            let result = applicable
                .substitution
                .apply_type(types, callable.result())?;
            let (result_capability, target) = borrow_result(types, result)
                .ok_or(InstanceSelectionError::InvalidCoercionSignature(*member))?;
            if result_capability != capability {
                return Err(InstanceSelectionError::InvalidCoercionSignature(*member));
            }
            let callable_requirements = normalize_requirements(
                graph,
                types,
                &applicable.substitution,
                callable.requirements(),
            )?;
            if !requirements_hold(
                graph,
                types,
                conformances,
                copyabilities,
                assumptions,
                &callable_requirements,
                &TypeSubstitution::default(),
            )? {
                continue;
            }
            selected.push(CoercionCandidate {
                target,
                selection: StaticSelection::new(
                    StaticDispatch::Direct(*member),
                    applicable.generic_arguments.clone(),
                ),
            });
        }
    }
    Ok(selected)
}

#[allow(clippy::too_many_arguments)]
fn applicable_instances(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    conformances: &ConformanceTable,
    copyabilities: &mut CopyabilityTable,
    table: &InstanceOperationTable,
    assumptions: &[CheckedRequirement],
    target: TypeId,
) -> Result<Vec<ApplicableInstance>, InstanceSelectionError> {
    if !is_concrete_type(types, target)? {
        return Ok(Vec::new());
    }
    let mut applicable = Vec::new();
    for instance in table.candidates(types, target).unwrap_or_default() {
        let entry = table
            .entries()
            .get(*instance)
            .ok_or(InstanceSelectionError::MissingInstance(*instance))?;
        let Some(bindings) = match_type_pattern(types, entry.target(), target)? else {
            continue;
        };
        let mut substitution = TypeSubstitution::default();
        for refinement in entry.refinements() {
            substitution.bind_generic(refinement.parameter(), refinement.ty());
        }
        for (parameter, ty) in bindings.iter() {
            substitution.bind_generic(parameter, ty);
        }
        let generic_arguments = selected_generic_arguments(types, entry, &substitution)?;
        if requirements_hold(
            graph,
            types,
            conformances,
            copyabilities,
            assumptions,
            entry.requirements(),
            &substitution,
        )? {
            applicable.push(ApplicableInstance {
                instance: *instance,
                substitution,
                generic_arguments,
            });
        }
    }
    Ok(applicable)
}

fn selected_generic_arguments(
    types: &mut TypeStore,
    entry: &super::CheckedInstanceOperations,
    substitution: &TypeSubstitution,
) -> Result<GenericArguments, InstanceSelectionError> {
    let mut arguments = Vec::with_capacity(entry.generic_parameters().len());
    for parameter in entry.generic_parameters() {
        let generic = types
            .intern(TypeKind::GenericParameter(*parameter))
            .map_err(|_| InstanceSelectionError::IncompleteGeneric(*parameter))?;
        let ty = substitution.apply_type(types, generic)?;
        if matches!(types.get(ty), Some(TypeKind::GenericParameter(actual)) if actual == parameter)
        {
            return Err(InstanceSelectionError::IncompleteGeneric(*parameter));
        }
        arguments.push(GenericArgument::new(*parameter, ty));
    }
    GenericArguments::new(arguments)
        .map_err(|duplicate| InstanceSelectionError::DuplicateGeneric(duplicate.parameter()))
}

#[allow(clippy::too_many_arguments)]
fn requirements_hold(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    conformances: &ConformanceTable,
    copyabilities: &mut CopyabilityTable,
    assumptions: &[CheckedRequirement],
    requirements: &[CheckedRequirement],
    substitution: &TypeSubstitution,
) -> Result<bool, InstanceSelectionError> {
    for requirement in requirements {
        let predicate = substitute_predicate(types, substitution, requirement.predicate())?;
        let proven = match &predicate {
            CheckedPredicate::Copy(ty) => {
                copyabilities
                    .classify(graph, types, *ty)
                    .map_err(InstanceSelectionError::Copyability)?
                    == Copyability::Copy
            }
            _ => proves_predicate(types, conformances, assumptions, &predicate)?,
        };
        if !proven {
            return Ok(false);
        }
    }
    Ok(true)
}

fn borrow_result(types: &TypeStore, result: TypeId) -> Option<(BorrowCapability, TypeId)> {
    match types.get(result)? {
        TypeKind::Borrow {
            capability,
            referent,
        } => Some((*capability, *referent)),
        _ => None,
    }
}

fn builtin_index_result(
    types: &TypeStore,
    target: TypeId,
    capability: BorrowCapability,
) -> Option<TypeId> {
    match types.get(target)? {
        TypeKind::FixedArray { element, .. } | TypeKind::Slice(element) => Some(*element),
        TypeKind::Builtin(nocter_model::BuiltinType::Str)
            if capability == BorrowCapability::Readonly =>
        {
            Some(types.builtin(nocter_model::BuiltinType::U8))
        }
        _ => None,
    }
}

fn callable_capability(capability: BorrowCapability) -> CallableCapability {
    match capability {
        BorrowCapability::Readonly => CallableCapability::Readonly,
        BorrowCapability::ReadWrite => CallableCapability::ReadWrite,
    }
}

fn visible_callable(
    graph: &DeclarationGraph,
    from: ModuleId,
    site: nocter_model::DeclarationSiteId,
) -> Result<bool, InstanceSelectionError> {
    let site = graph
        .declaration_sites()
        .get(site)
        .copied()
        .ok_or(InstanceSelectionError::MissingSite(site))?;
    Ok(graph.is_visible_from(site.visibility(), from, site.module()))
}
