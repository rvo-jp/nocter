use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use nocter_declarations::{
    CallableDeclaration, CallableKind, CallableOwner, DeclarationGraph, ExpansionCapability,
    ParameterOwner, ParameterRole,
};
use nocter_diagnostics::SourceDiagnostic;
use nocter_model::{
    AttachmentFamily, BorrowCapability, BuiltinType, CallableCapability, CallableId, InstanceId,
    Symbol, TypeId, TypeKind, TypeStore,
};
use nocter_source_index::{SemanticEntity, SourceIndex, SourceOrigin};

use super::contracts::{
    CheckedInstanceCoercion, CheckedInstanceComparison, CheckedInstanceExpansion,
    CheckedInstanceIndex, CheckedInstanceMember, CheckedInstanceMethod,
};
use super::diagnostic;
use super::model::{CheckedInstanceOperations, InstanceOperationTable};
use crate::declaration_patterns::DeclarationPatternTable;
use crate::type_relations::{SubstitutionError, type_patterns_overlap};

#[derive(Debug)]
pub enum InstanceOperationBuildError {
    Rule(SourceDiagnostic),
    Internal(InstanceOperationInternalError),
}

impl InstanceOperationBuildError {
    #[must_use]
    pub const fn source_diagnostic(&self) -> Option<&SourceDiagnostic> {
        match self {
            Self::Rule(diagnostic) => Some(diagnostic),
            Self::Internal(_) => None,
        }
    }
}

impl fmt::Display for InstanceOperationBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rule(diagnostic) => {
                write!(formatter, "{}: {}", diagnostic.code(), diagnostic.message())
            }
            Self::Internal(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for InstanceOperationBuildError {}

impl From<SourceDiagnostic> for InstanceOperationBuildError {
    fn from(diagnostic: SourceDiagnostic) -> Self {
        Self::Rule(diagnostic)
    }
}

impl From<InstanceOperationInternalError> for InstanceOperationBuildError {
    fn from(error: InstanceOperationInternalError) -> Self {
        Self::Internal(error)
    }
}

impl From<SubstitutionError> for InstanceOperationBuildError {
    fn from(error: SubstitutionError) -> Self {
        Self::Internal(InstanceOperationInternalError::Substitution(error))
    }
}

#[derive(Debug)]
pub enum InstanceOperationInternalError {
    InvalidTarget(nocter_model::TypeId),
    MissingInstance(InstanceId),
    MissingCallable(CallableId),
    MissingParameter(nocter_model::ParameterId),
    MissingReceiver(CallableId),
    InvalidMember(CallableId),
    MissingSource(SemanticEntity),
    Substitution(SubstitutionError),
}

impl fmt::Display for InstanceOperationInternalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTarget(target) => write!(formatter, "invalid instance target {target:?}"),
            Self::MissingInstance(instance) => write!(formatter, "missing instance {instance:?}"),
            Self::MissingCallable(callable) => write!(formatter, "missing callable {callable:?}"),
            Self::MissingParameter(parameter) => {
                write!(formatter, "missing parameter {parameter:?}")
            }
            Self::MissingReceiver(callable) => {
                write!(formatter, "missing coercion receiver for {callable:?}")
            }
            Self::InvalidMember(callable) => {
                write!(formatter, "invalid instance member {callable:?}")
            }
            Self::MissingSource(entity) => write!(formatter, "missing source for {entity:?}"),
            Self::Substitution(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for InstanceOperationInternalError {}

/// Builds and validates the single normalized index for all instance-owned operations.
///
/// # Errors
///
/// Returns a source-backed error when instance target patterns overlap and an internal error when
/// the declaration graph, type store, or source index are inconsistent.
#[cfg(test)]
pub(super) fn build_instance_operation_table(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    source_index: &SourceIndex,
) -> Result<InstanceOperationTable, InstanceOperationBuildError> {
    let patterns = DeclarationPatternTable::build(graph, types)?;
    let operations = crate::admitted_operations::AdmittedOperations::new(graph, None);
    build_instance_operation_table_from_ids(
        graph,
        types,
        source_index,
        &patterns,
        operations.instances(),
    )
}

pub(crate) fn build_instance_operation_table_from_ids(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    source_index: &SourceIndex,
    patterns: &DeclarationPatternTable,
    instances: &[InstanceId],
) -> Result<InstanceOperationTable, InstanceOperationBuildError> {
    let declarations = graph.declarations();
    let mut entries = BTreeMap::<InstanceId, CheckedInstanceOperations>::new();
    let mut by_family = BTreeMap::<AttachmentFamily, Vec<InstanceId>>::new();
    let mut method_names_by_family = BTreeMap::<AttachmentFamily, BTreeSet<Symbol>>::new();

    for id in instances {
        let id = *id;
        let instance = declarations
            .instances()
            .get(id)
            .ok_or(InstanceOperationInternalError::MissingInstance(id))?;
        let pattern = patterns
            .instance(id)
            .ok_or(InstanceOperationInternalError::MissingInstance(id))?;
        let target = pattern.target();
        let family = AttachmentFamily::of(types, target)
            .ok_or(InstanceOperationInternalError::InvalidTarget(target))?;
        if let Some(previous) = by_family.get(&family) {
            for previous in previous {
                let previous_entry = entries
                    .get(previous)
                    .ok_or(InstanceOperationInternalError::MissingInstance(*previous))?;
                if type_patterns_overlap(types, previous_entry.target(), target)? {
                    let previous = declarations
                        .instances()
                        .get(*previous)
                        .ok_or(InstanceOperationInternalError::MissingInstance(*previous))?;
                    return Err(diagnostic::overlapping(
                        site_origin(source_index, instance.site())?,
                        site_origin(source_index, previous.site())?,
                    )
                    .into());
                }
            }
        }
        let members = build_member_contracts(
            graph,
            types,
            source_index,
            id,
            target,
            instance.members(),
            pattern.substitution(),
        )?;
        validate_coercion_identities(types, source_index, &members, pattern.substitution())?;
        for member in &members {
            if let CheckedInstanceMember::Method(method) = member {
                method_names_by_family
                    .entry(family)
                    .or_default()
                    .insert(method.name());
            }
        }
        let previous = entries.insert(
            id,
            CheckedInstanceOperations::new(
                target,
                instance.generic_parameters(),
                pattern.lexical().refinements().to_vec(),
                pattern.lexical().requirements().to_vec(),
                members,
            ),
        );
        debug_assert!(previous.is_none());
        by_family.entry(family).or_default().push(id);
    }

    Ok(InstanceOperationTable::new(
        entries,
        by_family
            .into_iter()
            .map(|(family, instances)| (family, instances.into_boxed_slice()))
            .collect(),
        method_names_by_family
            .into_iter()
            .map(|(family, names)| (family, names.into_iter().collect()))
            .collect(),
    ))
}

fn build_member_contracts(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    source_index: &SourceIndex,
    instance: InstanceId,
    target: TypeId,
    members: &[CallableId],
    substitution: &crate::type_relations::TypeSubstitution,
) -> Result<Box<[CheckedInstanceMember]>, InstanceOperationBuildError> {
    members
        .iter()
        .copied()
        .map(|member| {
            build_member_contract(
                graph,
                types,
                source_index,
                instance,
                target,
                member,
                substitution,
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Vec::into_boxed_slice)
}

fn build_member_contract(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    source_index: &SourceIndex,
    instance: InstanceId,
    target: TypeId,
    member: CallableId,
    substitution: &crate::type_relations::TypeSubstitution,
) -> Result<CheckedInstanceMember, InstanceOperationBuildError> {
    let declarations = graph.declarations();
    let callable = declarations
        .callables()
        .get(member)
        .ok_or(InstanceOperationInternalError::MissingCallable(member))?;
    if callable.owner() != CallableOwner::Instance(instance) {
        return Err(InstanceOperationInternalError::InvalidMember(member).into());
    }
    let receiver_id = callable
        .receiver()
        .ok_or(InstanceOperationInternalError::MissingReceiver(member))?;
    let receiver = declarations.parameters().get(receiver_id).ok_or(
        InstanceOperationInternalError::MissingParameter(receiver_id),
    )?;
    if receiver.owner() != ParameterOwner::Callable(member) {
        return Err(InstanceOperationInternalError::InvalidMember(member).into());
    }
    let ParameterRole::Receiver(receiver_capability) = receiver.role() else {
        return invalid_member_signature(source_index, member);
    };
    if substitution.apply_type(types, receiver.ty())? != target {
        return invalid_member_signature(source_index, member);
    }
    let parameters = callable
        .parameters()
        .iter()
        .copied()
        .enumerate()
        .map(|(position, parameter)| {
            let declaration = declarations
                .parameters()
                .get(parameter)
                .ok_or(InstanceOperationInternalError::MissingParameter(parameter))?;
            let positioned = matches!(
                declaration.role(),
                ParameterRole::Ordinary { position: actual }
                    | ParameterRole::ArgumentPack { position: actual }
                    if actual == position
            );
            if declaration.owner() != ParameterOwner::Callable(member) || !positioned {
                return Err(InstanceOperationInternalError::InvalidMember(member));
            }
            Ok((parameter, declaration.ty(), declaration.role()))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let site = callable.site();
    let contract = match callable.kind() {
        CallableKind::Method => {
            let name = callable
                .name()
                .ok_or(InstanceOperationInternalError::InvalidMember(member))?;
            CheckedInstanceMember::Method(CheckedInstanceMethod::new(
                member,
                site,
                name,
                receiver_capability,
            ))
        }
        CallableKind::Coercion => build_coercion_contract(
            types,
            source_index,
            member,
            callable,
            receiver_capability,
            &parameters,
        )?,
        CallableKind::Equality | CallableKind::Ordering => build_comparison_contract(
            types,
            source_index,
            member,
            callable,
            receiver_capability,
            &parameters,
            target,
            substitution,
        )?,
        CallableKind::Index => build_index_contract(
            types,
            source_index,
            member,
            callable,
            receiver_capability,
            &parameters,
        )?,
        CallableKind::Expansion => build_expansion_contract(
            source_index,
            member,
            callable,
            receiver_capability,
            &parameters,
        )?,
        CallableKind::Function
        | CallableKind::Primitive
        | CallableKind::ConstructionFunction
        | CallableKind::Literal(_) => {
            return Err(InstanceOperationInternalError::InvalidMember(member).into());
        }
    };
    Ok(contract)
}

fn build_coercion_contract(
    types: &TypeStore,
    source_index: &SourceIndex,
    member: CallableId,
    callable: &CallableDeclaration,
    receiver_capability: CallableCapability,
    parameters: &[(nocter_model::ParameterId, TypeId, ParameterRole)],
) -> Result<CheckedInstanceMember, InstanceOperationBuildError> {
    let Some(receiver_capability) = borrow_capability(receiver_capability) else {
        return invalid_member_signature(source_index, member);
    };
    if !parameters.is_empty() || !callable.generic_parameters().is_empty() {
        return invalid_member_signature(source_index, member);
    }
    let Some((result_capability, result_target)) = borrow_type(types, callable.result()) else {
        return invalid_member_signature(source_index, member);
    };
    Ok(CheckedInstanceMember::Coercion(
        CheckedInstanceCoercion::new(
            member,
            callable.site(),
            receiver_capability,
            result_capability,
            result_target,
            callable.requirements(),
        ),
    ))
}

#[allow(clippy::too_many_arguments)]
fn build_comparison_contract(
    types: &mut TypeStore,
    source_index: &SourceIndex,
    member: CallableId,
    callable: &CallableDeclaration,
    receiver_capability: CallableCapability,
    parameters: &[(nocter_model::ParameterId, TypeId, ParameterRole)],
    target: TypeId,
    substitution: &crate::type_relations::TypeSubstitution,
) -> Result<CheckedInstanceMember, InstanceOperationBuildError> {
    if receiver_capability != CallableCapability::Readonly
        || parameters.len() != 1
        || parameters[0].2 != (ParameterRole::Ordinary { position: 0 })
        || !callable.generic_parameters().is_empty()
    {
        return invalid_member_signature(source_index, member);
    }
    let parameter = substitution.apply_type(types, parameters[0].1)?;
    let result = substitution.apply_type(types, callable.result())?;
    if borrow_type(types, parameter) != Some((BorrowCapability::Readonly, target))
        || result != types.builtin(BuiltinType::Bool)
    {
        return invalid_member_signature(source_index, member);
    }
    let contract = CheckedInstanceComparison::new(member, callable.site(), callable.requirements());
    Ok(if callable.kind() == CallableKind::Equality {
        CheckedInstanceMember::Equality(contract)
    } else {
        CheckedInstanceMember::Ordering(contract)
    })
}

fn build_index_contract(
    types: &TypeStore,
    source_index: &SourceIndex,
    member: CallableId,
    callable: &CallableDeclaration,
    receiver_capability: CallableCapability,
    parameters: &[(nocter_model::ParameterId, TypeId, ParameterRole)],
) -> Result<CheckedInstanceMember, InstanceOperationBuildError> {
    let Some(capability) = borrow_capability(receiver_capability) else {
        return invalid_member_signature(source_index, member);
    };
    if parameters.len() != 1
        || parameters[0].2 != (ParameterRole::Ordinary { position: 0 })
        || !callable.generic_parameters().is_empty()
    {
        return invalid_member_signature(source_index, member);
    }
    let Some((result_capability, result)) = borrow_type(types, callable.result()) else {
        return invalid_member_signature(source_index, member);
    };
    if result_capability != capability {
        return invalid_member_signature(source_index, member);
    }
    Ok(CheckedInstanceMember::Index(CheckedInstanceIndex::new(
        member,
        callable.site(),
        capability,
        parameters[0].1,
        result,
        callable.requirements(),
    )))
}

fn build_expansion_contract(
    source_index: &SourceIndex,
    member: CallableId,
    callable: &CallableDeclaration,
    receiver_capability: CallableCapability,
    parameters: &[(nocter_model::ParameterId, TypeId, ParameterRole)],
) -> Result<CheckedInstanceMember, InstanceOperationBuildError> {
    if !parameters.is_empty() || !callable.generic_parameters().is_empty() {
        return invalid_member_signature(source_index, member);
    }
    Ok(CheckedInstanceMember::Expansion(
        CheckedInstanceExpansion::new(
            member,
            callable.site(),
            expansion_capability(receiver_capability),
            callable.result(),
            callable.requirements(),
        ),
    ))
}

fn validate_coercion_identities(
    types: &mut TypeStore,
    source_index: &SourceIndex,
    members: &[CheckedInstanceMember],
    substitution: &crate::type_relations::TypeSubstitution,
) -> Result<(), InstanceOperationBuildError> {
    let mut identities =
        BTreeMap::<(BorrowCapability, BorrowCapability, TypeId), CallableId>::new();
    for member in members {
        let CheckedInstanceMember::Coercion(contract) = member else {
            continue;
        };
        let target = substitution.apply_type(types, contract.target())?;
        if let Some(previous) = identities.insert(
            (
                contract.receiver_capability(),
                contract.result_capability(),
                target,
            ),
            contract.callable(),
        ) {
            return Err(diagnostic::duplicate_coercion(
                entity_origin(source_index, SemanticEntity::Callable(contract.callable()))?,
                entity_origin(source_index, SemanticEntity::Callable(previous))?,
            )
            .into());
        }
    }
    Ok(())
}

fn invalid_member_signature(
    source_index: &SourceIndex,
    member: CallableId,
) -> Result<CheckedInstanceMember, InstanceOperationBuildError> {
    Err(diagnostic::invalid_signature(entity_origin(
        source_index,
        SemanticEntity::Callable(member),
    )?)
    .into())
}

const fn borrow_capability(capability: CallableCapability) -> Option<BorrowCapability> {
    match capability {
        CallableCapability::Readonly => Some(BorrowCapability::Readonly),
        CallableCapability::ReadWrite => Some(BorrowCapability::ReadWrite),
        CallableCapability::Owned => None,
    }
}

const fn expansion_capability(capability: CallableCapability) -> ExpansionCapability {
    match capability {
        CallableCapability::Readonly => ExpansionCapability::Readonly,
        CallableCapability::ReadWrite => ExpansionCapability::ReadWrite,
        CallableCapability::Owned => ExpansionCapability::Owned,
    }
}

fn borrow_type(types: &TypeStore, ty: TypeId) -> Option<(BorrowCapability, TypeId)> {
    let TypeKind::Borrow {
        capability,
        referent,
    } = types.get(ty)?
    else {
        return None;
    };
    Some((*capability, *referent))
}

fn site_origin(
    source_index: &SourceIndex,
    site: nocter_model::DeclarationSiteId,
) -> Result<SourceOrigin, InstanceOperationInternalError> {
    let entity = SemanticEntity::DeclarationSite(site);
    crate::diagnostic_projection::declaration_origin(source_index, entity)
        .ok_or(InstanceOperationInternalError::MissingSource(entity))
}

fn entity_origin(
    source_index: &SourceIndex,
    entity: SemanticEntity,
) -> Result<SourceOrigin, InstanceOperationInternalError> {
    crate::diagnostic_projection::declaration_origin(source_index, entity)
        .ok_or(InstanceOperationInternalError::MissingSource(entity))
}
