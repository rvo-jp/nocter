use std::collections::HashSet;

use nocter_declarations::{InterfaceApplication, StructuralCapability};
use nocter_model::{ConformanceId, TypeId, TypeKind, TypeStore};

use super::model::ConformanceTable;
use super::overlap::match_pattern;
use super::predicate::{CheckedPredicate, CheckedRequirement, substitute_predicate};
use crate::type_relations::{SubstitutionError, TypeSubstitution};

/// One explicit conformance selected for an exact interface application and subject type.
pub(crate) struct ConformanceSelection {
    declaration: ConformanceId,
    substitution: TypeSubstitution,
}

impl ConformanceSelection {
    pub(crate) const fn declaration(&self) -> ConformanceId {
        self.declaration
    }

    pub(crate) const fn substitution(&self) -> &TypeSubstitution {
        &self.substitution
    }
}

pub(crate) fn proves(
    types: &mut TypeStore,
    table: &ConformanceTable,
    assumptions: &[CheckedRequirement],
    intrinsic_facts: &[CheckedPredicate],
    predicate: &CheckedPredicate,
) -> Result<bool, SubstitutionError> {
    Prover::new(types, table, assumptions, intrinsic_facts).prove(predicate)
}

/// Selects the explicit conformance that proves one exact interface application.
///
/// Lexical assumptions may prove conditional requirements but are not returned as invented
/// conformance declarations. Program-wide overlap validation guarantees at most one match.
pub(crate) fn select_conformance(
    types: &mut TypeStore,
    table: &ConformanceTable,
    assumptions: &[CheckedRequirement],
    intrinsic_facts: &[CheckedPredicate],
    subject: TypeId,
    application: &InterfaceApplication,
) -> Result<Option<ConformanceSelection>, SubstitutionError> {
    let mut prover = Prover::new(types, table, assumptions, intrinsic_facts);
    let root = CheckedPredicate::Capability {
        subject,
        capability: StructuralCapability::Interface(application.clone()),
    };
    if !prover.active.insert(root.clone()) {
        return Ok(None);
    }
    let selected = prover.select_interface(subject, application)?;
    prover.active.remove(&root);
    Ok(selected)
}

struct Prover<'program> {
    types: &'program mut TypeStore,
    table: &'program ConformanceTable,
    assumptions: &'program [CheckedRequirement],
    intrinsic_facts: &'program [CheckedPredicate],
    active: HashSet<CheckedPredicate>,
    proven: HashSet<CheckedPredicate>,
}

impl<'program> Prover<'program> {
    fn new(
        types: &'program mut TypeStore,
        table: &'program ConformanceTable,
        assumptions: &'program [CheckedRequirement],
        intrinsic_facts: &'program [CheckedPredicate],
    ) -> Self {
        Self {
            types,
            table,
            assumptions,
            intrinsic_facts,
            active: HashSet::new(),
            proven: HashSet::new(),
        }
    }

    fn prove(&mut self, predicate: &CheckedPredicate) -> Result<bool, SubstitutionError> {
        if self
            .assumptions
            .iter()
            .any(|assumption| assumption.predicate() == predicate)
            || self.intrinsic_facts.contains(predicate)
            || self.proven.contains(predicate)
        {
            return Ok(true);
        }
        if !self.active.insert(predicate.clone()) {
            return Ok(false);
        }
        let result = match predicate {
            CheckedPredicate::TypeEquality { left, right } => left == right,
            CheckedPredicate::Capability {
                subject,
                capability: StructuralCapability::Callable(expected),
            } => matches!(
                self.types.get(*subject),
                Some(TypeKind::Callable(actual)) if actual == expected
            ),
            CheckedPredicate::Capability {
                subject,
                capability: StructuralCapability::Interface(application),
            } => self.select_interface(*subject, application)?.is_some(),
            CheckedPredicate::Copy(_)
            | CheckedPredicate::Equality(_)
            | CheckedPredicate::Ordering(_)
            | CheckedPredicate::Index { .. }
            | CheckedPredicate::Coercion { .. }
            | CheckedPredicate::Expansion { .. } => false,
        };
        self.active.remove(predicate);
        if result {
            self.proven.insert(predicate.clone());
        }
        Ok(result)
    }

    fn select_interface(
        &mut self,
        subject: TypeId,
        application: &InterfaceApplication,
    ) -> Result<Option<ConformanceSelection>, SubstitutionError> {
        let candidates = self.table.candidates(application.interface()).to_vec();
        for declaration in candidates {
            let conformance = self
                .table
                .entries()
                .get(declaration)
                .ok_or(SubstitutionError::InvalidStore)?;
            let Some(matched) = match_pattern(
                self.types,
                conformance.interface(),
                conformance.target(),
                application,
                subject,
            )?
            else {
                continue;
            };
            let mut substitution = TypeSubstitution::default();
            for refinement in conformance.refinements() {
                substitution.bind_generic(refinement.parameter(), refinement.ty());
            }
            substitution.extend(&matched);
            let requirements = conformance.requirements().to_vec();
            for requirement in requirements {
                let predicate =
                    substitute_predicate(self.types, &substitution, requirement.predicate())?;
                if !self.prove(&predicate)? {
                    return Ok(None);
                }
            }
            return Ok(Some(ConformanceSelection {
                declaration,
                substitution,
            }));
        }
        Ok(None)
    }
}
