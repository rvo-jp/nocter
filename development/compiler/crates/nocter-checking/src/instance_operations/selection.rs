use std::collections::HashSet;
use std::fmt;

use nocter_declarations::{CallableKind, DeclarationGraph, ParameterRole};
use nocter_model::{BorrowCapability, CallableCapability, ModuleId, TypeId, TypeKind, TypeStore};

use super::InstanceOperationTable;
use crate::conformance::normalize_requirements;
use crate::type_relations::{
    SubstitutionError, TypeSubstitution, is_concrete_type, match_type_pattern,
};
use crate::{
    CheckedPredicate, CheckedRequirement, ConformanceTable, CopyabilityError, CopyabilityTable,
    GenericArgument, GenericArguments, StaticDispatch, StaticSelection,
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

pub(super) struct ApplicableInstance {
    pub(super) instance: nocter_model::InstanceId,
    pub(super) substitution: TypeSubstitution,
    pub(super) generic_arguments: GenericArguments,
}

pub(crate) struct CoercionCandidate {
    pub(super) target: TypeId,
    pub(super) receiver_capability: BorrowCapability,
    pub(super) result_capability: BorrowCapability,
    pub(super) selection: StaticSelection,
}

impl CoercionCandidate {
    pub(crate) const fn target(&self) -> TypeId {
        self.target
    }

    pub(crate) const fn receiver_capability(&self) -> BorrowCapability {
        self.receiver_capability
    }

    pub(crate) const fn result_capability(&self) -> BorrowCapability {
        self.result_capability
    }

    pub(crate) const fn selection(&self) -> &StaticSelection {
        &self.selection
    }
}

#[derive(Debug)]
pub enum InstanceSelectionError {
    MissingInstance(nocter_model::InstanceId),
    MissingConformance(nocter_model::ConformanceId),
    MissingInterface(nocter_model::InterfaceId),
    MissingCallable(nocter_model::CallableId),
    MissingNominal(nocter_model::NominalTypeId),
    MissingParameter(nocter_model::ParameterId),
    MissingSite(nocter_model::DeclarationSiteId),
    MissingVariant(nocter_model::VariantId),
    UnknownType(TypeId),
    InvalidIndexSignature(nocter_model::CallableId),
    InvalidCoercionSignature(nocter_model::CallableId),
    InvalidComparisonSignature(nocter_model::CallableId),
    InvalidMethodSignature(nocter_model::CallableId),
    InvalidInterfaceMethod(nocter_model::InterfaceId),
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
            Self::MissingConformance(conformance) => {
                write!(formatter, "missing conformance {conformance:?}")
            }
            Self::MissingInterface(interface) => {
                write!(formatter, "missing interface {interface:?}")
            }
            Self::MissingCallable(callable) => write!(formatter, "missing callable {callable:?}"),
            Self::MissingNominal(nominal) => write!(formatter, "missing nominal type {nominal:?}"),
            Self::MissingParameter(parameter) => {
                write!(formatter, "missing parameter {parameter:?}")
            }
            Self::MissingSite(site) => write!(formatter, "missing declaration site {site:?}"),
            Self::MissingVariant(variant) => write!(formatter, "missing enum variant {variant:?}"),
            Self::UnknownType(ty) => write!(formatter, "missing type {ty:?}"),
            Self::InvalidIndexSignature(callable) => {
                write!(formatter, "invalid index operation signature {callable:?}")
            }
            Self::InvalidCoercionSignature(callable) => {
                write!(formatter, "invalid coercion signature {callable:?}")
            }
            Self::InvalidComparisonSignature(callable) => {
                write!(formatter, "invalid comparison signature {callable:?}")
            }
            Self::InvalidMethodSignature(callable) => {
                write!(formatter, "invalid method signature {callable:?}")
            }
            Self::InvalidInterfaceMethod(interface) => {
                write!(
                    formatter,
                    "invalid method index for interface {interface:?}"
                )
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

/// Stateful authority for one body's instance-operation selection and recursive requirement proof.
pub(crate) struct InstanceOperationSelector<'program> {
    pub(super) graph: &'program DeclarationGraph,
    pub(super) types: &'program mut TypeStore,
    pub(super) conformances: &'program ConformanceTable,
    pub(super) copyabilities: &'program mut CopyabilityTable,
    pub(super) table: &'program InstanceOperationTable,
    pub(super) assumptions: &'program [CheckedRequirement],
    pub(super) from: ModuleId,
    pub(super) active: HashSet<CheckedPredicate>,
}

impl<'program> InstanceOperationSelector<'program> {
    pub(crate) fn new(
        graph: &'program DeclarationGraph,
        types: &'program mut TypeStore,
        conformances: &'program ConformanceTable,
        copyabilities: &'program mut CopyabilityTable,
        table: &'program InstanceOperationTable,
        assumptions: &'program [CheckedRequirement],
        from: ModuleId,
    ) -> Self {
        Self {
            graph,
            types,
            conformances,
            copyabilities,
            table,
            assumptions,
            from,
            active: HashSet::new(),
        }
    }

    /// Selects every visible, requirement-satisfied direct index operation on one receiver type.
    ///
    /// Candidate order has no semantic meaning. The caller checks the index expression once and
    /// rejects a retained set other than exactly one.
    pub(crate) fn select_index_operations(
        &mut self,
        target: TypeId,
        capability: BorrowCapability,
    ) -> Result<Vec<IndexOperationCandidate>, InstanceSelectionError> {
        let mut selected = self.structural_index_operations(target, capability)?;
        selected.extend(self.select_instance_index_operations(target, capability)?);
        Ok(selected)
    }

    fn structural_index_operations(
        &self,
        target: TypeId,
        capability: BorrowCapability,
    ) -> Result<Vec<IndexOperationCandidate>, InstanceSelectionError> {
        let mut selected = Vec::new();
        for assumption in self.assumptions {
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
            let (result_capability, referent) = borrow_result(self.types, *result).ok_or(
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

    fn select_instance_index_operations(
        &mut self,
        target: TypeId,
        capability: BorrowCapability,
    ) -> Result<Vec<IndexOperationCandidate>, InstanceSelectionError> {
        let mut selected = Vec::new();
        for applicable in self.applicable_instances(target)? {
            let members = self
                .table
                .entries()
                .get(applicable.instance)
                .ok_or(InstanceSelectionError::MissingInstance(applicable.instance))?
                .members()
                .to_vec();
            for member in members {
                let callable = self
                    .graph
                    .declarations()
                    .callables()
                    .get(member)
                    .ok_or(InstanceSelectionError::MissingCallable(member))?;
                if callable.kind() != CallableKind::Index
                    || !visible_callable(self.graph, self.from, callable.site())?
                {
                    continue;
                }
                let receiver = callable
                    .receiver()
                    .and_then(|receiver| self.graph.declarations().parameters().get(receiver))
                    .ok_or(InstanceSelectionError::InvalidIndexSignature(member))?;
                if receiver.role() != ParameterRole::Receiver(callable_capability(capability))
                    || callable.parameters().len() != 1
                    || !callable.generic_parameters().is_empty()
                {
                    continue;
                }
                let parameter_id = callable.parameters()[0];
                let result_id = callable.result();
                let requirements = callable.requirements().to_vec();
                let parameter = self
                    .graph
                    .declarations()
                    .parameters()
                    .get(parameter_id)
                    .ok_or(InstanceSelectionError::MissingParameter(parameter_id))?;
                let index = applicable
                    .substitution
                    .apply_type(self.types, parameter.ty())?;
                let result = applicable.substitution.apply_type(self.types, result_id)?;
                let (result_capability, referent) = borrow_result(self.types, result)
                    .ok_or(InstanceSelectionError::InvalidIndexSignature(member))?;
                if result_capability != capability {
                    return Err(InstanceSelectionError::InvalidIndexSignature(member));
                }
                let callable_requirements = normalize_requirements(
                    self.graph,
                    self.types,
                    &applicable.substitution,
                    &requirements,
                )?;
                if !self.requirements_hold(&callable_requirements, &TypeSubstitution::default())? {
                    continue;
                }
                selected.push(IndexOperationCandidate {
                    index,
                    result: referent,
                    operation: Some(StaticSelection::new(
                        StaticDispatch::Direct(member),
                        applicable.generic_arguments.clone(),
                    )),
                    receiver_coercion: None,
                });
            }
        }
        Ok(selected)
    }

    /// Selects one receiver coercion followed by one built-in or source-defined index operation.
    pub(crate) fn select_coerced_index_operations(
        &mut self,
        source: TypeId,
        capability: BorrowCapability,
    ) -> Result<Vec<IndexOperationCandidate>, InstanceSelectionError> {
        let coercions = self.select_borrow_coercions(source, capability, capability)?;
        let mut selected = Vec::new();
        for coercion in coercions {
            if let Some(result) = builtin_index_result(self.types, coercion.target, capability) {
                selected.push(IndexOperationCandidate {
                    index: self.types.builtin(nocter_model::BuiltinType::Usize),
                    result,
                    operation: None,
                    receiver_coercion: Some(coercion.selection),
                });
                continue;
            }
            for mut candidate in self.select_index_operations(coercion.target, capability)? {
                candidate.receiver_coercion = Some(coercion.selection.clone());
                selected.push(candidate);
            }
        }
        Ok(selected)
    }

    pub(super) fn select_coercions(
        &mut self,
        source: TypeId,
        capability: BorrowCapability,
    ) -> Result<Vec<CoercionCandidate>, InstanceSelectionError> {
        let mut selected = self.structural_coercions(source, capability);
        for applicable in self.applicable_instances(source)? {
            let members = self
                .table
                .entries()
                .get(applicable.instance)
                .ok_or(InstanceSelectionError::MissingInstance(applicable.instance))?
                .members()
                .to_vec();
            for member in members {
                let callable = self
                    .graph
                    .declarations()
                    .callables()
                    .get(member)
                    .ok_or(InstanceSelectionError::MissingCallable(member))?;
                if callable.kind() != CallableKind::Coercion
                    || !visible_callable(self.graph, self.from, callable.site())?
                {
                    continue;
                }
                let receiver = callable
                    .receiver()
                    .and_then(|receiver| self.graph.declarations().parameters().get(receiver))
                    .ok_or(InstanceSelectionError::InvalidCoercionSignature(member))?;
                if receiver.role() != ParameterRole::Receiver(callable_capability(capability))
                    || !callable.parameters().is_empty()
                    || !callable.generic_parameters().is_empty()
                {
                    continue;
                }
                let result_id = callable.result();
                let requirements = callable.requirements().to_vec();
                let result = applicable.substitution.apply_type(self.types, result_id)?;
                let (result_capability, target) = borrow_result(self.types, result)
                    .ok_or(InstanceSelectionError::InvalidCoercionSignature(member))?;
                let callable_requirements = normalize_requirements(
                    self.graph,
                    self.types,
                    &applicable.substitution,
                    &requirements,
                )?;
                if !self.requirements_hold(&callable_requirements, &TypeSubstitution::default())? {
                    continue;
                }
                selected.push(CoercionCandidate {
                    target,
                    receiver_capability: capability,
                    result_capability,
                    selection: StaticSelection::new(
                        StaticDispatch::Direct(member),
                        applicable.generic_arguments.clone(),
                    ),
                });
            }
        }
        Ok(selected)
    }

    fn structural_coercions(
        &self,
        source: TypeId,
        capability: BorrowCapability,
    ) -> Vec<CoercionCandidate> {
        self.assumptions
            .iter()
            .filter_map(|assumption| {
                let CheckedPredicate::Coercion {
                    source: required_source,
                    target,
                } = assumption.predicate()
                else {
                    return None;
                };
                let (required_capability, required_owner) =
                    borrow_result(self.types, *required_source)?;
                let (result_capability, target) = borrow_result(self.types, *target)?;
                (required_capability == capability && required_owner == source).then(|| {
                    CoercionCandidate {
                        target,
                        receiver_capability: capability,
                        result_capability,
                        selection: StaticSelection::new(
                            StaticDispatch::StructuralRequirement(assumption.declaration()),
                            GenericArguments::default(),
                        ),
                    }
                })
            })
            .collect()
    }

    /// Selects one borrow coercion under capability weakening and minimum-authority priority.
    pub(crate) fn select_borrow_coercions(
        &mut self,
        source: TypeId,
        source_capability: BorrowCapability,
        target_capability: BorrowCapability,
    ) -> Result<Vec<CoercionCandidate>, InstanceSelectionError> {
        let preferred_receiver = match (source_capability, target_capability) {
            (BorrowCapability::ReadWrite, BorrowCapability::Readonly) => BorrowCapability::Readonly,
            (capability, _) => capability,
        };
        let preferred = self
            .select_coercions(source, preferred_receiver)?
            .into_iter()
            .filter(|candidate| candidate.result_capability == target_capability)
            .collect::<Vec<_>>();
        if !preferred.is_empty()
            || source_capability != BorrowCapability::ReadWrite
            || target_capability != BorrowCapability::Readonly
        {
            return Ok(preferred);
        }
        Ok(self
            .select_coercions(source, BorrowCapability::ReadWrite)?
            .into_iter()
            .filter(|candidate| candidate.result_capability == BorrowCapability::Readonly)
            .collect())
    }

    pub(super) fn applicable_instances(
        &mut self,
        target: TypeId,
    ) -> Result<Vec<ApplicableInstance>, InstanceSelectionError> {
        if !is_concrete_type(self.types, target)? {
            return Ok(Vec::new());
        }
        let instances = self
            .table
            .candidates(self.types, target)
            .unwrap_or_default()
            .to_vec();
        let mut applicable = Vec::new();
        for instance in instances {
            let entry = self
                .table
                .entries()
                .get(instance)
                .ok_or(InstanceSelectionError::MissingInstance(instance))?;
            let pattern = entry.target();
            let refinements = entry.refinements().to_vec();
            let generic_parameters = entry.generic_parameters().to_vec();
            let requirements = entry.requirements().to_vec();
            let Some(bindings) = match_type_pattern(self.types, pattern, target)? else {
                continue;
            };
            let mut substitution = TypeSubstitution::default();
            for refinement in refinements {
                substitution.bind_generic(refinement.parameter(), refinement.ty());
            }
            for (parameter, ty) in bindings.iter() {
                substitution.bind_generic(parameter, ty);
            }
            let generic_arguments =
                selected_generic_arguments(self.types, &generic_parameters, &substitution)?;
            if self.requirements_hold(&requirements, &substitution)? {
                applicable.push(ApplicableInstance {
                    instance,
                    substitution,
                    generic_arguments,
                });
            }
        }
        Ok(applicable)
    }
}

pub(super) fn selected_generic_arguments(
    types: &mut TypeStore,
    generic_parameters: &[nocter_model::GenericParameterId],
    substitution: &TypeSubstitution,
) -> Result<GenericArguments, InstanceSelectionError> {
    let mut arguments = Vec::with_capacity(generic_parameters.len());
    for parameter in generic_parameters {
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

pub(crate) fn retain_direct_candidates(candidates: &mut Vec<IndexOperationCandidate>) {
    if candidates.iter().any(IndexOperationCandidate::is_direct) {
        candidates.retain(IndexOperationCandidate::is_direct);
    }
}

pub(super) fn borrow_result(
    types: &TypeStore,
    result: TypeId,
) -> Option<(BorrowCapability, TypeId)> {
    match types.get(result)? {
        TypeKind::Borrow {
            capability,
            referent,
        } => Some((*capability, *referent)),
        _ => None,
    }
}

pub(super) fn builtin_index_result(
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

pub(super) fn visible_callable(
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
