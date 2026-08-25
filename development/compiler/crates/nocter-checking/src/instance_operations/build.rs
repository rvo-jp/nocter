use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use nocter_declarations::{CallableKind, DeclarationGraph, ParameterRole};
use nocter_diagnostics::SourceDiagnostic;
use nocter_model::{
    AttachmentFamily, CallableCapability, CallableId, InstanceId, Symbol, TypeId, TypeStore,
};
use nocter_source_index::{SemanticEntity, SourceIndex, SourceOrigin};

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
    MissingReceiver(CallableId),
    MissingSource(SemanticEntity),
    Substitution(SubstitutionError),
}

impl fmt::Display for InstanceOperationInternalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTarget(target) => write!(formatter, "invalid instance target {target:?}"),
            Self::MissingInstance(instance) => write!(formatter, "missing instance {instance:?}"),
            Self::MissingCallable(callable) => write!(formatter, "missing callable {callable:?}"),
            Self::MissingReceiver(callable) => {
                write!(formatter, "missing coercion receiver for {callable:?}")
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
pub fn build_instance_operation_table(
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
        validate_coercion_identities(
            graph,
            types,
            source_index,
            instance.members(),
            pattern.substitution(),
        )?;
        for member in instance.members() {
            let callable = declarations
                .callables()
                .get(*member)
                .ok_or(InstanceOperationInternalError::MissingCallable(*member))?;
            if callable.kind() == CallableKind::Method {
                let name = callable
                    .name()
                    .ok_or(InstanceOperationInternalError::MissingCallable(*member))?;
                method_names_by_family
                    .entry(family)
                    .or_default()
                    .insert(name);
            }
        }
        let previous = entries.insert(
            id,
            CheckedInstanceOperations::new(
                target,
                instance.generic_parameters(),
                pattern.lexical().refinements().to_vec(),
                pattern.lexical().requirements().to_vec(),
                instance.members(),
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

fn validate_coercion_identities(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    source_index: &SourceIndex,
    members: &[CallableId],
    substitution: &crate::type_relations::TypeSubstitution,
) -> Result<(), InstanceOperationBuildError> {
    let declarations = graph.declarations();
    let mut identities = BTreeMap::<(CallableCapability, TypeId), CallableId>::new();
    for member in members.iter().copied() {
        let callable = declarations
            .callables()
            .get(member)
            .ok_or(InstanceOperationInternalError::MissingCallable(member))?;
        if callable.kind() != CallableKind::Coercion {
            continue;
        }
        let receiver = callable
            .receiver()
            .and_then(|receiver| declarations.parameters().get(receiver))
            .ok_or(InstanceOperationInternalError::MissingReceiver(member))?;
        let ParameterRole::Receiver(capability) = receiver.role() else {
            return Err(InstanceOperationInternalError::MissingReceiver(member).into());
        };
        let target = substitution.apply_type(types, callable.result())?;
        if let Some(previous) = identities.insert((capability, target), member) {
            return Err(diagnostic::duplicate_coercion(
                entity_origin(source_index, SemanticEntity::Callable(member))?,
                entity_origin(source_index, SemanticEntity::Callable(previous))?,
            )
            .into());
        }
    }
    Ok(())
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
