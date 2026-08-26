use nocter_declarations::DeclarationGraph;
use nocter_model::{AssociatedTypeId, RequirementId, TypeStore};
use nocter_source_index::{SemanticEntity, SourceIndex, SourceOrigin};

use super::build::{InterfaceImplementationBuildError, InterfaceImplementationInternalError};
use super::diagnostic;
use super::model::{CheckedInterfaceImplementation, InterfaceImplementationTable};
use super::predicate::normalize_requirements;
use super::selection::proves;
use crate::type_relations::TypeSubstitution;

pub(super) fn validate_associated_bounds(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    source_index: &SourceIndex,
    table: &InterfaceImplementationTable,
) -> Result<(), InterfaceImplementationBuildError> {
    for (id, interface_implementation) in table.entries() {
        let declaration = graph
            .declarations()
            .interface_implementations()
            .get(*id)
            .ok_or(InterfaceImplementationInternalError::MissingInterfaceImplementation(*id))?;
        let interface = graph
            .declarations()
            .interfaces()
            .get(interface_implementation.interface().interface())
            .ok_or(InterfaceImplementationInternalError::MissingInterface(
                interface_implementation.interface().interface(),
            ))?;
        let substitution = interface_implementation_substitution(graph, interface_implementation)?;
        let mut validation = AssociatedValidationContext {
            graph,
            types,
            source_index,
            table,
            interface_implementation,
            interface_implementation_site: declaration.site(),
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
    table: &'program InterfaceImplementationTable,
    interface_implementation: &'program CheckedInterfaceImplementation,
    interface_implementation_site: nocter_model::DeclarationSiteId,
    substitution: &'program TypeSubstitution,
}

impl AssociatedValidationContext<'_> {
    fn validate(
        &mut self,
        associated: AssociatedTypeId,
    ) -> Result<(), InterfaceImplementationBuildError> {
        let declaration = self
            .graph
            .declarations()
            .associated_types()
            .get(associated)
            .ok_or(InterfaceImplementationInternalError::MissingAssociatedType(
                associated,
            ))?;
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
                self.interface_implementation.requirements(),
                &[],
                requirement.predicate(),
            )? {
                return Err(diagnostic::unsatisfied_associated_bound(
                    source_origin(
                        self.source_index,
                        SemanticEntity::DeclarationSite(self.interface_implementation_site),
                    )?,
                    requirement_origin(self.source_index, requirement.declaration())?,
                )
                .into());
            }
        }
        Ok(())
    }
}

fn interface_implementation_substitution(
    graph: &DeclarationGraph,
    interface_implementation: &CheckedInterfaceImplementation,
) -> Result<TypeSubstitution, InterfaceImplementationInternalError> {
    let interface_id = interface_implementation.interface().interface();
    let interface = graph.declarations().interfaces().get(interface_id).ok_or(
        InterfaceImplementationInternalError::MissingInterface(interface_id),
    )?;
    let mut substitution = TypeSubstitution::default();
    substitution.set_interface_self(interface_id, interface_implementation.target());
    for (parameter, argument) in interface
        .generic_parameters()
        .iter()
        .zip(interface_implementation.interface().arguments())
    {
        substitution.bind_generic(*parameter, *argument);
    }
    for binding in interface_implementation.associated_types() {
        substitution.bind_associated(binding.declaration(), binding.ty());
    }
    Ok(substitution)
}

fn requirement_origin(
    source_index: &SourceIndex,
    requirement: RequirementId,
) -> Result<SourceOrigin, InterfaceImplementationInternalError> {
    source_origin(source_index, SemanticEntity::Requirement(requirement))
}

fn source_origin(
    source_index: &SourceIndex,
    entity: SemanticEntity,
) -> Result<SourceOrigin, InterfaceImplementationInternalError> {
    crate::diagnostic_projection::declaration_origin(source_index, entity)
        .ok_or(InterfaceImplementationInternalError::MissingSource(entity))
}
