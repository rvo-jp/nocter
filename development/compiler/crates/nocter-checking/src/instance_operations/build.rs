use std::collections::BTreeMap;
use std::fmt;

use nocter_declarations::DeclarationGraph;
use nocter_diagnostics::SourceDiagnostic;
use nocter_model::{ArenaBuilder, InstanceId, TypeStore};
use nocter_source_index::{SemanticEntity, SourceIndex, SourceOrigin, SourceRole};

use super::diagnostic;
use super::model::{CheckedInstanceOperations, InstanceFamily, InstanceOperationTable, family};
use crate::conformance::normalize_requirements;
use crate::pattern_requirements::PatternRequirements;
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
    MissingSource(SemanticEntity),
    Substitution(SubstitutionError),
}

impl fmt::Display for InstanceOperationInternalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTarget(target) => write!(formatter, "invalid instance target {target:?}"),
            Self::MissingInstance(instance) => write!(formatter, "missing instance {instance:?}"),
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
    let declarations = graph.declarations();
    let mut entries = ArenaBuilder::<InstanceId, CheckedInstanceOperations>::new();
    let mut by_family = BTreeMap::<InstanceFamily, Vec<InstanceId>>::new();

    for (id, instance) in declarations.instances().iter() {
        let pattern_requirements = PatternRequirements::collect(graph, instance.requirements())?;
        let pattern_substitution = pattern_requirements.substitution();
        let target = pattern_substitution.apply_type(types, instance.target())?;
        let family =
            family(types, target).ok_or(InstanceOperationInternalError::InvalidTarget(target))?;
        if let Some(previous) = by_family.get(&family) {
            for previous in previous {
                let previous_entry = entries
                    .get(*previous)
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
        let refinements =
            pattern_requirements.normalized_refinements(types, instance.generic_parameters())?;
        let requirements = normalize_requirements(
            graph,
            types,
            &pattern_substitution,
            pattern_requirements.retained(),
        )?;
        let actual = entries.insert(CheckedInstanceOperations::new(
            target,
            instance.generic_parameters(),
            refinements,
            requirements,
            instance.members(),
        ));
        debug_assert_eq!(actual, id);
        by_family.entry(family).or_default().push(id);
    }

    Ok(InstanceOperationTable::new(
        entries.finish(),
        by_family
            .into_iter()
            .map(|(family, instances)| (family, instances.into_boxed_slice()))
            .collect(),
    ))
}

fn site_origin(
    source_index: &SourceIndex,
    site: nocter_model::DeclarationSiteId,
) -> Result<SourceOrigin, InstanceOperationInternalError> {
    source_index
        .bindings_for(SemanticEntity::DeclarationSite(site))
        .iter()
        .find(|binding| {
            matches!(
                binding.role(),
                SourceRole::Declaration | SourceRole::Implementation
            )
        })
        .map(|binding| binding.origin())
        .ok_or(InstanceOperationInternalError::MissingSource(
            SemanticEntity::DeclarationSite(site),
        ))
}
