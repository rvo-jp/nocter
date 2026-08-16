use nocter_declarations::{BodyOwner, CallableOwner, DeclarationGraph};
use nocter_model::TypeStore;

use crate::conformance::normalize_requirements;
use crate::type_relations::{SubstitutionError, TypeSubstitution};
use crate::{BodySource, CheckedRequirement, ConformanceTable, InstanceOperationTable};

/// Collects the normalized lexical predicate environment for one checked body.
pub(super) fn body_assumptions(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    conformances: &ConformanceTable,
    instance_operations: &InstanceOperationTable,
    source: BodySource<'_>,
) -> Result<Vec<CheckedRequirement>, SubstitutionError> {
    let BodyOwner::Callable(callable_id) = source.owner() else {
        return Ok(Vec::new());
    };
    let callable = graph
        .declarations()
        .callables()
        .get(callable_id)
        .ok_or(SubstitutionError::InvalidStore)?;
    let mut assumptions = Vec::new();
    let mut substitution = TypeSubstitution::default();
    match callable.owner() {
        CallableOwner::Instance(instance) => {
            let entry = instance_operations
                .entries()
                .get(instance)
                .ok_or(SubstitutionError::InvalidStore)?;
            assumptions.extend_from_slice(entry.requirements());
            for refinement in entry.refinements() {
                substitution.bind_generic(refinement.parameter(), refinement.ty());
            }
        }
        CallableOwner::Conformance(conformance) => {
            let entry = conformances
                .entries()
                .get(conformance)
                .ok_or(SubstitutionError::InvalidStore)?;
            assumptions.extend_from_slice(entry.requirements());
            for refinement in entry.refinements() {
                substitution.bind_generic(refinement.parameter(), refinement.ty());
            }
        }
        CallableOwner::Interface(interface) => {
            let interface = graph
                .declarations()
                .interfaces()
                .get(interface)
                .ok_or(SubstitutionError::InvalidStore)?;
            assumptions.extend(normalize_requirements(
                graph,
                types,
                &substitution,
                interface.requirements(),
            )?);
        }
        CallableOwner::Module(_) | CallableOwner::Construction(_) => {}
    }
    assumptions.extend(normalize_requirements(
        graph,
        types,
        &substitution,
        callable.requirements(),
    )?);
    Ok(assumptions)
}
