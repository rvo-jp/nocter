use nocter_declarations::DeclarationGraph;
use nocter_model::{AssociatedTypeId, RequirementId};
use nocter_source_index::{DiagnosticOrigins, SemanticEntity, SourceOrigin};

use super::build::{InterfaceImplementationBuildError, InterfaceImplementationInternalError};
use super::diagnostic;
use super::model::{CheckedInterfaceImplementation, InterfaceImplementationTable};
use super::predicate::normalize_requirements;
use super::selection::proves;
use crate::instance_operations::{
    InstanceOperationSelector, InstanceOperationTable, InstanceSelectionContext,
};
use crate::type_relations::TypeSubstitution;

pub(super) fn validate_associated_bounds(
    graph: &DeclarationGraph,
    types: &mut nocter_model::TypeTransaction,
    source_index: DiagnosticOrigins<'_>,
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

pub(crate) fn validate_interface_prerequisites(
    graph: &DeclarationGraph,
    types: &mut nocter_model::TypeTransaction,
    source_index: DiagnosticOrigins<'_>,
    table: &InterfaceImplementationTable,
    instance_operations: &InstanceOperationTable,
    copyabilities: &crate::CopyabilityTable,
) -> Result<(), InterfaceImplementationBuildError> {
    for (implementation_id, implementation) in table.entries() {
        let interface_id = implementation.interface().interface();
        let capability = graph.interface_capabilities().get(interface_id).ok_or(
            InterfaceImplementationInternalError::MissingInterface(interface_id),
        )?;
        if capability.direct_prerequisites().is_empty() {
            continue;
        }
        let declaration = graph
            .declarations()
            .interface_implementations()
            .get(*implementation_id)
            .ok_or(
                InterfaceImplementationInternalError::MissingInterfaceImplementation(
                    *implementation_id,
                ),
            )?;
        let substitution = interface_implementation_substitution(graph, implementation)?;
        let requirements = normalize_requirements(
            graph,
            types,
            &substitution,
            capability.direct_prerequisites(),
        )?;
        let mut copy_transaction = copyabilities.transaction();
        let context = InstanceSelectionContext::for_prerequisite_validation(
            graph,
            table,
            instance_operations,
            implementation.requirements(),
        );
        let mut selector = InstanceOperationSelector::new(context, types, &mut copy_transaction);
        for requirement in &requirements {
            if !selector
                .requirements_hold(
                    std::slice::from_ref(requirement),
                    &TypeSubstitution::default(),
                )
                .map_err(|error| {
                    InterfaceImplementationInternalError::PrerequisiteSelection(Box::new(error))
                })?
            {
                return Err(diagnostic::unsatisfied_prerequisite(
                    source_origin(
                        source_index,
                        SemanticEntity::DeclarationSite(declaration.site()),
                    )?,
                    requirement_origin(source_index, requirement.declaration())?,
                )
                .into());
            }
        }
    }
    Ok(())
}

struct AssociatedValidationContext<'program> {
    graph: &'program DeclarationGraph,
    types: &'program mut nocter_model::TypeTransaction,
    source_index: DiagnosticOrigins<'program>,
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
    source_index: DiagnosticOrigins<'_>,
    requirement: RequirementId,
) -> Result<SourceOrigin, InterfaceImplementationInternalError> {
    source_origin(source_index, SemanticEntity::Requirement(requirement))
}

fn source_origin(
    source_index: DiagnosticOrigins<'_>,
    entity: SemanticEntity,
) -> Result<SourceOrigin, InterfaceImplementationInternalError> {
    source_index
        .declaration(entity)
        .ok_or(InterfaceImplementationInternalError::MissingSource(entity))
}
