use nocter_declarations::{BodyOwner, CallableOwner, DeclarationGraph, InterfaceApplication};
use nocter_model::{Arena, ArenaBuilder, BodyId, CapabilityEvidenceId, TypeKind};

use crate::copyability::CopyProofs;
use crate::declaration_patterns::DeclarationPatternTable;
use crate::interface_implementation::normalize_requirements;
use crate::type_relations::{SubstitutionError, TypeSubstitution};
use crate::{CheckedPredicate, CheckedRequirement, RequirementDerivation};

/// The complete lexical proof environment for one body.
///
/// Declared requirements retain source identities for structural dispatch. Intrinsic facts are
/// language truths, such as an interface default body's `Self` satisfying that same interface,
/// and deliberately cannot masquerade as authored requirements.
#[derive(Debug)]
pub(crate) struct BodyAssumptions {
    declared: Box<[BodyRequirement]>,
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
}

/// Program-wide immutable authority for every structural dispatch fact admitted into a body.
#[derive(Debug)]
pub(crate) struct CapabilityEvidenceTable {
    entries: Arena<CapabilityEvidenceId, CapabilityEvidence>,
}

#[derive(Clone, Debug)]
pub struct CapabilityEvidence {
    derivations: Box<[RequirementDerivation]>,
    predicate: CheckedPredicate,
}

impl CapabilityEvidence {
    #[must_use]
    pub const fn derivations(&self) -> &[RequirementDerivation] {
        &self.derivations
    }

    #[must_use]
    pub const fn predicate(&self) -> &CheckedPredicate {
        &self.predicate
    }
}

/// A declaration predicate admitted into one body with an exact immutable evidence identity.
#[derive(Clone, Debug)]
pub(crate) struct BodyRequirement {
    requirement: CheckedRequirement,
    evidence: CapabilityEvidenceId,
}

impl BodyRequirement {
    pub(crate) fn root(&self) -> nocter_model::RequirementId {
        self.requirement.derivations()[0].root()
    }

    pub(crate) const fn predicate(&self) -> &CheckedPredicate {
        self.requirement.predicate()
    }

    pub(crate) const fn evidence(&self) -> CapabilityEvidenceId {
        self.evidence
    }
}

impl crate::interface_implementation::RequirementPredicate for BodyRequirement {
    fn predicate(&self) -> &CheckedPredicate {
        self.predicate()
    }
}

impl BodyAssumptionTable {
    pub(crate) fn build(
        graph: &DeclarationGraph,
        types: &mut nocter_model::TypeTransaction,
        declaration_patterns: &DeclarationPatternTable,
    ) -> Result<(Self, CapabilityEvidenceTable), SubstitutionError> {
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
        Ok((
            Self {
                entries: entries.finish(),
            },
            CapabilityEvidenceTable {
                entries: evidence.finish(),
            },
        ))
    }

    pub(crate) fn get(&self, body: BodyId) -> Option<&BodyAssumptions> {
        self.entries.get(body)
    }
}

impl CapabilityEvidenceTable {
    pub(crate) fn get(&self, id: CapabilityEvidenceId) -> Option<&CapabilityEvidence> {
        self.entries.get(id)
    }

    pub(crate) const fn entries(&self) -> &Arena<CapabilityEvidenceId, CapabilityEvidence> {
        &self.entries
    }
}

impl BodyAssumptions {
    pub(crate) fn declared(&self) -> &[BodyRequirement] {
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
            .map(BodyRequirement::predicate)
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
) -> Vec<BodyRequirement> {
    let mut result = Vec::with_capacity(declared.len());
    for requirement in declared {
        let predicate = requirement.predicate().clone();
        let id = evidence.insert(CapabilityEvidence {
            derivations: requirement.derivations().into(),
            predicate: predicate.clone(),
        });
        result.push(BodyRequirement {
            requirement,
            evidence: id,
        });
    }
    result
}
