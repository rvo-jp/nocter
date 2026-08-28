use nocter_declarations::{BodyOwner, CallableOwner, DeclarationGraph, InterfaceApplication};
use nocter_model::{Arena, ArenaBuilder, BodyId, CapabilityEvidenceId, TypeKind};

use crate::copyability::CopyProofs;
use crate::declaration_patterns::DeclarationPatternTable;
use crate::interface_implementation::normalize_requirements;
use crate::type_relations::{SubstitutionError, TypeSubstitution};
use crate::{CheckedPredicate, CheckedRequirement};

/// The complete lexical proof environment for one body.
///
/// Declared requirements retain source identities for structural dispatch. Intrinsic facts are
/// language truths, such as an interface default body's `Self` satisfying that same interface,
/// and deliberately cannot masquerade as authored requirements.
#[derive(Debug)]
pub(crate) struct BodyAssumptions {
    declared: Box<[CheckedRequirement]>,
    intrinsic: Box<[CheckedPredicate]>,
    copy_proofs: CopyProofs,
}

/// Sole normalized lexical-proof authority for declaration bodies.
///
/// Declaration patterns remain preparation inputs only. This table freezes the exact facts each
/// body checker and editor completion query consumes, so neither consumer re-normalizes callable
/// or interface requirements.
#[derive(Debug)]
pub(crate) struct BodyAssumptionTable {
    entries: Arena<BodyId, BodyAssumptions>,
    evidence: Arena<CapabilityEvidenceId, CapabilityEvidence>,
}

#[derive(Clone, Debug)]
pub(crate) struct CapabilityEvidence {
    predicate: CheckedPredicate,
}

impl CapabilityEvidence {
    pub(crate) const fn predicate(&self) -> &CheckedPredicate {
        &self.predicate
    }
}

impl BodyAssumptionTable {
    pub(crate) fn build(
        graph: &DeclarationGraph,
        types: &mut nocter_model::TypeTransaction,
        declaration_patterns: &DeclarationPatternTable,
    ) -> Result<Self, SubstitutionError> {
        let mut entries = ArenaBuilder::new();
        let mut evidence = ArenaBuilder::new();
        for (body_id, body) in graph.declarations().bodies().iter() {
            let owner = body.owner();
            let actual = entries.insert(normalize_body_assumptions(
                graph,
                types,
                declaration_patterns,
                owner,
                &mut evidence,
            )?);
            if actual != body_id {
                return Err(SubstitutionError::InvalidStore);
            }
        }
        Ok(Self {
            entries: entries.finish(),
            evidence: evidence.finish(),
        })
    }

    pub(crate) fn get(&self, body: BodyId) -> Option<&BodyAssumptions> {
        self.entries.get(body)
    }

    pub(crate) fn evidence(&self, id: CapabilityEvidenceId) -> Option<&CapabilityEvidence> {
        self.evidence.get(id)
    }
}

impl BodyAssumptions {
    pub(crate) fn declared(&self) -> &[CheckedRequirement] {
        &self.declared
    }

    pub(crate) fn intrinsic(&self) -> &[CheckedPredicate] {
        &self.intrinsic
    }

    pub(super) fn copy_proofs(&self) -> &CopyProofs {
        &self.copy_proofs
    }
}

/// Collects the normalized lexical predicate environment for one checked body.
fn normalize_body_assumptions(
    graph: &DeclarationGraph,
    types: &mut nocter_model::TypeTransaction,
    declaration_patterns: &DeclarationPatternTable,
    owner: BodyOwner,
    evidence: &mut ArenaBuilder<CapabilityEvidenceId, CapabilityEvidence>,
) -> Result<BodyAssumptions, SubstitutionError> {
    let BodyOwner::Callable(callable_id) = owner else {
        return Ok(BodyAssumptions {
            declared: Box::new([]),
            intrinsic: Box::new([]),
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
        owner @ CallableOwner::Instance(_) => {
            let entry = declaration_patterns
                .lexical(owner)
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
            intrinsic.push(CheckedPredicate::Interface {
                subject,
                application: InterfaceApplication::new(interface_id, arguments),
                associated_types: Box::new([]),
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
    let declared = freeze_declared_evidence(declared, evidence);
    let copy_proofs = CopyProofs::from_predicates(
        types,
        declared
            .iter()
            .map(CheckedRequirement::predicate)
            .chain(intrinsic.iter()),
    );
    Ok(BodyAssumptions {
        declared: declared.into_boxed_slice(),
        intrinsic: intrinsic.into_boxed_slice(),
        copy_proofs,
    })
}

fn freeze_declared_evidence(
    declared: Vec<CheckedRequirement>,
    evidence: &mut ArenaBuilder<CapabilityEvidenceId, CapabilityEvidence>,
) -> Vec<CheckedRequirement> {
    let mut result = Vec::with_capacity(declared.len());
    for requirement in declared {
        let declaration = requirement.declaration();
        let predicate = requirement.predicate().clone();
        if result
            .iter()
            .any(|existing: &CheckedRequirement| existing.predicate() == &predicate)
        {
            continue;
        }
        let id = evidence.insert(CapabilityEvidence {
            predicate: predicate.clone(),
        });
        result.push(CheckedRequirement::with_evidence(
            declaration,
            predicate,
            id,
        ));
    }
    result
}
