use nocter_declarations::DeclarationGraph;
use nocter_model::{AssociatedTypeId, RequirementId, TypeStore};
use nocter_source_index::{SemanticEntity, SourceIndex, SourceOrigin, SourceRole};

use super::build::{ConformanceBuildError, ConformanceInternalError};
use super::diagnostic;
use super::model::{CheckedConformance, ConformanceTable};
use super::predicate::normalize_requirements;
use super::selection::proves;
use crate::type_relations::TypeSubstitution;

pub(super) fn validate_associated_bounds(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    source_index: &SourceIndex,
    table: &ConformanceTable,
) -> Result<(), ConformanceBuildError> {
    for (id, conformance) in table.entries().iter() {
        let declaration = graph
            .declarations()
            .conformances()
            .get(id)
            .ok_or(ConformanceInternalError::MissingConformance(id))?;
        let interface = graph
            .declarations()
            .interfaces()
            .get(conformance.interface().interface())
            .ok_or(ConformanceInternalError::MissingInterface(
                conformance.interface().interface(),
            ))?;
        let substitution = conformance_substitution(graph, conformance)?;
        let mut validation = AssociatedValidationContext {
            graph,
            types,
            source_index,
            table,
            conformance,
            conformance_site: declaration.site(),
            substitution: &substitution,
        };
        for associated in interface.associated_types() {
            validation.validate(*associated)?;
        }
    }
    Ok(())
}

struct AssociatedValidationContext<'program> {
    graph: &'program DeclarationGraph,
    types: &'program mut TypeStore,
    source_index: &'program SourceIndex,
    table: &'program ConformanceTable,
    conformance: &'program CheckedConformance,
    conformance_site: nocter_model::DeclarationSiteId,
    substitution: &'program TypeSubstitution,
}

impl AssociatedValidationContext<'_> {
    fn validate(&mut self, associated: AssociatedTypeId) -> Result<(), ConformanceBuildError> {
        let declaration = self
            .graph
            .declarations()
            .associated_types()
            .get(associated)
            .ok_or(ConformanceInternalError::MissingAssociatedType(associated))?;
        let requirements = normalize_requirements(
            self.graph,
            self.types,
            self.substitution,
            declaration.bounds(),
        )?;
        for requirement in requirements {
            if !proves(
                self.types,
                self.table,
                self.conformance.requirements(),
                requirement.predicate(),
            )? {
                return Err(diagnostic::unsatisfied_associated_bound(
                    source_origin(
                        self.source_index,
                        SemanticEntity::DeclarationSite(self.conformance_site),
                    )?,
                    requirement_origin(self.source_index, requirement.declaration())?,
                )
                .into());
            }
        }
        Ok(())
    }
}

fn conformance_substitution(
    graph: &DeclarationGraph,
    conformance: &CheckedConformance,
) -> Result<TypeSubstitution, ConformanceInternalError> {
    let interface_id = conformance.interface().interface();
    let interface = graph
        .declarations()
        .interfaces()
        .get(interface_id)
        .ok_or(ConformanceInternalError::MissingInterface(interface_id))?;
    let mut substitution = TypeSubstitution::default();
    substitution.set_interface_self(interface_id, conformance.target());
    for (parameter, argument) in interface
        .generic_parameters()
        .iter()
        .zip(conformance.interface().arguments())
    {
        substitution.bind_generic(*parameter, *argument);
    }
    for binding in conformance.associated_types() {
        substitution.bind_associated(binding.declaration(), binding.ty());
    }
    Ok(substitution)
}

fn requirement_origin(
    source_index: &SourceIndex,
    requirement: RequirementId,
) -> Result<SourceOrigin, ConformanceInternalError> {
    source_origin(source_index, SemanticEntity::Requirement(requirement))
}

fn source_origin(
    source_index: &SourceIndex,
    entity: SemanticEntity,
) -> Result<SourceOrigin, ConformanceInternalError> {
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
        .ok_or(ConformanceInternalError::MissingSource(entity))
}
