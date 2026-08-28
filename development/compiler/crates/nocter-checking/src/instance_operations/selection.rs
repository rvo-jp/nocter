use std::collections::HashSet;
use std::fmt;

use nocter_declarations::DeclarationGraph;
use nocter_model::{BorrowCapability, TypeId, TypeKind, TypeStore};

use super::{CheckedInstanceMember, InstanceOperationTable};
use crate::interface_implementation::normalize_requirements;
use crate::type_relations::{
    SubstitutionError, TypeSubstitution, TypeUnificationError, collect_generic_parameters,
    match_type_pattern,
};
use crate::{
    CheckedPredicate, CheckedRequirement, CopyabilityError, GenericArgument, GenericArguments,
    InterfaceImplementationTable, StaticDispatch, StaticSelection,
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
    MissingInterfaceImplementation(nocter_model::InterfaceImplementationId),
    MissingInterface(nocter_model::InterfaceId),
    MissingCallable(nocter_model::CallableId),
    MissingNominal(nocter_model::NominalTypeId),
    MissingParameter(nocter_model::ParameterId),
    MissingSite(nocter_model::DeclarationSiteId),
    MissingVariant(nocter_model::VariantId),
    SourceAccess(nocter_frontend_bindings::SourceAccessError),
    UnknownType(TypeId),
    InvalidIndexSignature(nocter_model::CallableId),
    InvalidCoercionSignature(nocter_model::CallableId),
    InvalidComparisonSignature(nocter_model::CallableId),
    InvalidExpansionSignature(nocter_model::CallableId),
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
            Self::MissingInterfaceImplementation(interface_implementation) => {
                write!(
                    formatter,
                    "missing interface implementation {interface_implementation:?}"
                )
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
            Self::SourceAccess(error) => error.fmt(formatter),
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
            Self::InvalidExpansionSignature(callable) => {
                write!(formatter, "invalid expansion signature {callable:?}")
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
#[derive(Clone, Copy)]
enum CandidateVisibility<'program> {
    Lexical(crate::SourceAccessContext<'program>),
    CheckedEvidence,
}

#[derive(Clone, Copy)]
pub(crate) struct InstanceSelectionContext<'program> {
    graph: &'program DeclarationGraph,
    interface_implementations: &'program InterfaceImplementationTable,
    table: &'program InstanceOperationTable,
    assumptions: &'program [CheckedRequirement],
    intrinsic_facts: &'program [CheckedPredicate],
    visibility: CandidateVisibility<'program>,
}

impl<'program> InstanceSelectionContext<'program> {
    pub(crate) const fn new(
        graph: &'program DeclarationGraph,
        interface_implementations: &'program InterfaceImplementationTable,
        table: &'program InstanceOperationTable,
        assumptions: &'program [CheckedRequirement],
        intrinsic_facts: &'program [CheckedPredicate],
        from: crate::SourceAccessContext<'program>,
    ) -> Self {
        Self {
            graph,
            interface_implementations,
            table,
            assumptions,
            intrinsic_facts,
            visibility: CandidateVisibility::Lexical(from),
        }
    }

    /// Builds the closed specialization context for evidence already admitted by checking.
    ///
    /// Concrete specialization has no lexical module and cannot repeat source visibility.
    pub(crate) const fn for_concrete_evidence(
        graph: &'program DeclarationGraph,
        interface_implementations: &'program InterfaceImplementationTable,
        table: &'program InstanceOperationTable,
    ) -> Self {
        Self {
            graph,
            interface_implementations,
            table,
            assumptions: &[],
            intrinsic_facts: &[],
            visibility: CandidateVisibility::CheckedEvidence,
        }
    }

    pub(crate) const fn for_prerequisite_validation(
        graph: &'program DeclarationGraph,
        interface_implementations: &'program InterfaceImplementationTable,
        table: &'program InstanceOperationTable,
        assumptions: &'program [CheckedRequirement],
    ) -> Self {
        Self {
            graph,
            interface_implementations,
            table,
            assumptions,
            intrinsic_facts: &[],
            visibility: CandidateVisibility::CheckedEvidence,
        }
    }
}

pub(crate) struct InstanceOperationSelector<'program> {
    pub(super) graph: &'program DeclarationGraph,
    pub(super) types: &'program mut nocter_model::TypeTransaction,
    pub(super) interface_implementations: &'program InterfaceImplementationTable,
    pub(super) copyabilities: &'program mut crate::copyability::CopyabilityTransaction,
    pub(super) table: &'program InstanceOperationTable,
    pub(super) assumptions: &'program [CheckedRequirement],
    pub(super) intrinsic_facts: &'program [CheckedPredicate],
    visibility: CandidateVisibility<'program>,
    pub(super) active: HashSet<CheckedPredicate>,
}

impl<'program> InstanceOperationSelector<'program> {
    pub(crate) fn new(
        context: InstanceSelectionContext<'program>,
        types: &'program mut nocter_model::TypeTransaction,
        copyabilities: &'program mut crate::copyability::CopyabilityTransaction,
    ) -> Self {
        Self {
            graph: context.graph,
            types,
            interface_implementations: context.interface_implementations,
            copyabilities,
            table: context.table,
            assumptions: context.assumptions,
            intrinsic_facts: context.intrinsic_facts,
            visibility: context.visibility,
            active: HashSet::new(),
        }
    }

    pub(super) fn callable_is_admissible(
        &self,
        site: nocter_model::DeclarationSiteId,
    ) -> Result<bool, InstanceSelectionError> {
        match self.visibility {
            CandidateVisibility::Lexical(from) => visible_callable(self.graph, from, site),
            CandidateVisibility::CheckedEvidence => Ok(true),
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
                    StaticDispatch::StructuralRequirement {
                        requirement: assumption.declaration(),
                        evidence: assumption
                            .evidence()
                            .expect("body requirement has frozen evidence"),
                    },
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
            let operations = self
                .table
                .entries()
                .get(&applicable.instance)
                .ok_or(InstanceSelectionError::MissingInstance(applicable.instance))?
                .members()
                .iter()
                .filter_map(|member| match member {
                    CheckedInstanceMember::Index(operation) => Some(operation.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            for operation in operations {
                if operation.capability() != capability
                    || !self.callable_is_admissible(operation.site())?
                {
                    continue;
                }
                let index = applicable
                    .substitution
                    .apply_type(self.types, operation.index())?;
                let result = applicable
                    .substitution
                    .apply_type(self.types, operation.result())?;
                let callable_requirements = normalize_requirements(
                    self.graph,
                    self.types,
                    &applicable.substitution,
                    operation.requirements(),
                )?;
                if !self.requirements_hold(&callable_requirements, &TypeSubstitution::default())? {
                    continue;
                }
                selected.push(IndexOperationCandidate {
                    index,
                    result,
                    operation: Some(StaticSelection::new(
                        StaticDispatch::Direct(operation.callable()),
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
            let coercions = self
                .table
                .entries()
                .get(&applicable.instance)
                .ok_or(InstanceSelectionError::MissingInstance(applicable.instance))?
                .members()
                .iter()
                .filter_map(|member| match member {
                    CheckedInstanceMember::Coercion(coercion) => Some(coercion.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            for coercion in coercions {
                if coercion.receiver_capability() != capability
                    || !self.callable_is_admissible(coercion.site())?
                {
                    continue;
                }
                let target = applicable
                    .substitution
                    .apply_type(self.types, coercion.target())?;
                let callable_requirements = normalize_requirements(
                    self.graph,
                    self.types,
                    &applicable.substitution,
                    coercion.requirements(),
                )?;
                if !self.requirements_hold(&callable_requirements, &TypeSubstitution::default())? {
                    continue;
                }
                selected.push(CoercionCandidate {
                    target,
                    receiver_capability: capability,
                    result_capability: coercion.result_capability(),
                    selection: StaticSelection::new(
                        StaticDispatch::Direct(coercion.callable()),
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
                            StaticDispatch::StructuralRequirement {
                                requirement: assumption.declaration(),
                                evidence: assumption
                                    .evidence()
                                    .expect("body requirement has frozen evidence"),
                            },
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
                .get(&instance)
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
            let Some(generic_arguments) = selected_instance_generic_arguments(
                self.types,
                target,
                &generic_parameters,
                &substitution,
            )?
            else {
                continue;
            };
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

fn selected_instance_generic_arguments(
    types: &mut nocter_model::TypeTransaction,
    target: TypeId,
    generic_parameters: &[nocter_model::GenericParameterId],
    substitution: &TypeSubstitution,
) -> Result<Option<GenericArguments>, InstanceSelectionError> {
    let receiver_parameters =
        collect_generic_parameters(types, [target]).map_err(|error| match error {
            TypeUnificationError::UnknownType(ty) => InstanceSelectionError::UnknownType(ty),
            TypeUnificationError::Conflict(_) | TypeUnificationError::RecursiveBinding { .. } => {
                InstanceSelectionError::Substitution(SubstitutionError::InvalidStore)
            }
        })?;
    let mut arguments = Vec::with_capacity(generic_parameters.len());
    for parameter in generic_parameters {
        let generic = types
            .intern(TypeKind::GenericParameter(*parameter))
            .map_err(|_| InstanceSelectionError::IncompleteGeneric(*parameter))?;
        let ty = substitution.apply_type(types, generic)?;
        if matches!(types.get(ty), Some(TypeKind::GenericParameter(actual)) if actual == parameter)
            && !receiver_parameters.contains(parameter)
        {
            return Ok(None);
        }
        arguments.push(GenericArgument::new(*parameter, ty));
    }
    GenericArguments::new(arguments)
        .map(Some)
        .map_err(|duplicate| InstanceSelectionError::DuplicateGeneric(duplicate.parameter()))
}

pub(crate) fn selected_generic_arguments(
    types: &mut nocter_model::TypeTransaction,
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

pub(super) fn visible_callable(
    graph: &DeclarationGraph,
    from: crate::SourceAccessContext<'_>,
    site: nocter_model::DeclarationSiteId,
) -> Result<bool, InstanceSelectionError> {
    match from.site_is_visible(graph, site) {
        Ok(visible) => Ok(visible),
        Err(crate::source_visibility::SourceVisibilityError::MissingSite(site)) => {
            Err(InstanceSelectionError::MissingSite(site))
        }
        Err(crate::source_visibility::SourceVisibilityError::Access(error)) => {
            Err(InstanceSelectionError::SourceAccess(error))
        }
    }
}
