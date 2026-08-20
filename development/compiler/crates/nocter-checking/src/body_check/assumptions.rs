use nocter_declarations::{
    BodyOwner, CallableOwner, DeclarationGraph, InterfaceApplication, StructuralCapability,
};
use nocter_model::{TypeKind, TypeStore};

use crate::conformance::normalize_requirements;
use crate::copyability::CopyProofs;
use crate::type_relations::{SubstitutionError, TypeSubstitution};
use crate::{
    BodySource, CheckedPredicate, CheckedRequirement, ConformanceTable, InstanceOperationTable,
};

/// The complete lexical proof environment for one body.
///
/// Declared requirements retain source identities for structural dispatch. Intrinsic facts are
/// language truths, such as an interface default body's `Self` satisfying that same interface,
/// and deliberately cannot masquerade as authored requirements.
pub(super) struct BodyAssumptions {
    declared: Vec<CheckedRequirement>,
    intrinsic: Vec<CheckedPredicate>,
    copy_proofs: CopyProofs,
}

impl BodyAssumptions {
    pub(super) fn declared(&self) -> &[CheckedRequirement] {
        &self.declared
    }

    pub(super) fn intrinsic(&self) -> &[CheckedPredicate] {
        &self.intrinsic
    }

    pub(super) fn copy_proofs(&self) -> &CopyProofs {
        &self.copy_proofs
    }
}

/// Collects the normalized lexical predicate environment for one checked body.
pub(super) fn body_assumptions(
    graph: &DeclarationGraph,
    types: &mut TypeStore,
    conformances: &ConformanceTable,
    instance_operations: &InstanceOperationTable,
    source: BodySource<'_>,
) -> Result<BodyAssumptions, SubstitutionError> {
    let BodyOwner::Callable(callable_id) = source.owner() else {
        return Ok(BodyAssumptions {
            declared: Vec::new(),
            intrinsic: Vec::new(),
            copy_proofs: CopyProofs::default(),
        });
    };
    let callable = graph
        .declarations()
        .callables()
        .get(callable_id)
        .ok_or(SubstitutionError::InvalidStore)?;
    let mut declared = Vec::new();
    let mut intrinsic = Vec::new();
    let mut substitution = TypeSubstitution::default();
    match callable.owner() {
        CallableOwner::Instance(instance) => {
            let entry = instance_operations
                .entries()
                .get(instance)
                .ok_or(SubstitutionError::InvalidStore)?;
            declared.extend_from_slice(entry.requirements());
            for refinement in entry.refinements() {
                substitution.bind_generic(refinement.parameter(), refinement.ty());
            }
        }
        CallableOwner::Conformance(conformance) => {
            let entry = conformances
                .entries()
                .get(conformance)
                .ok_or(SubstitutionError::InvalidStore)?;
            declared.extend_from_slice(entry.requirements());
            for refinement in entry.refinements() {
                substitution.bind_generic(refinement.parameter(), refinement.ty());
            }
        }
        CallableOwner::Interface(interface_id) => {
            let interface = graph
                .declarations()
                .interfaces()
                .get(interface_id)
                .ok_or(SubstitutionError::InvalidStore)?;
            declared.extend(normalize_requirements(
                graph,
                types,
                &substitution,
                interface.requirements(),
            )?);
            let subject = types
                .intern(TypeKind::InterfaceSelf(interface_id))
                .map_err(|_| SubstitutionError::InvalidStore)?;
            let arguments = interface
                .generic_parameters()
                .iter()
                .map(|parameter| {
                    types
                        .intern(TypeKind::GenericParameter(*parameter))
                        .map_err(|_| SubstitutionError::InvalidStore)
                })
                .collect::<Result<Vec<_>, _>>()?;
            intrinsic.push(CheckedPredicate::Capability {
                subject,
                capability: StructuralCapability::Interface(InterfaceApplication::new(
                    interface_id,
                    arguments,
                )),
            });
        }
        CallableOwner::Module(_) | CallableOwner::Construction(_) => {}
    }
    declared.extend(normalize_requirements(
        graph,
        types,
        &substitution,
        callable.requirements(),
    )?);
    let copy_proofs = CopyProofs::from_predicates(
        types,
        declared
            .iter()
            .map(CheckedRequirement::predicate)
            .chain(intrinsic.iter()),
    );
    Ok(BodyAssumptions {
        declared,
        intrinsic,
        copy_proofs,
    })
}
