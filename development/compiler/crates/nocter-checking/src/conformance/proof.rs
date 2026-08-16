use std::collections::HashSet;

use nocter_declarations::StructuralCapability;
use nocter_model::{TypeKind, TypeStore};

use super::model::ConformanceTable;
use super::overlap::match_pattern;
use super::predicate::{CheckedPredicate, CheckedRequirement, substitute_predicate};
use crate::type_relations::SubstitutionError;

pub(super) fn proves(
    types: &mut TypeStore,
    table: &ConformanceTable,
    assumptions: &[CheckedRequirement],
    predicate: &CheckedPredicate,
) -> Result<bool, SubstitutionError> {
    Prover {
        types,
        table,
        assumptions,
        active: HashSet::new(),
        proven: HashSet::new(),
    }
    .prove(predicate)
}

struct Prover<'program> {
    types: &'program mut TypeStore,
    table: &'program ConformanceTable,
    assumptions: &'program [CheckedRequirement],
    active: HashSet<CheckedPredicate>,
    proven: HashSet<CheckedPredicate>,
}

impl Prover<'_> {
    fn prove(&mut self, predicate: &CheckedPredicate) -> Result<bool, SubstitutionError> {
        if self
            .assumptions
            .iter()
            .any(|assumption| assumption.predicate() == predicate)
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
            } => self.prove_interface(*subject, application)?,
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

    fn prove_interface(
        &mut self,
        subject: nocter_model::TypeId,
        application: &nocter_declarations::InterfaceApplication,
    ) -> Result<bool, SubstitutionError> {
        let candidates = self.table.candidates(application.interface()).to_vec();
        for candidate in candidates {
            let conformance = self
                .table
                .entries()
                .get(candidate)
                .ok_or(SubstitutionError::InvalidStore)?;
            let Some(substitution) = match_pattern(
                self.types,
                conformance.interface(),
                conformance.target(),
                application,
                subject,
            )?
            else {
                continue;
            };
            let requirements = conformance.requirements().to_vec();
            for requirement in requirements {
                let predicate =
                    substitute_predicate(self.types, &substitution, requirement.predicate())?;
                if !self.prove(&predicate)? {
                    return Ok(false);
                }
            }
            return Ok(true);
        }
        Ok(false)
    }
}
