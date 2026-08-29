use crate::checked::{
    ClosureAuthority, ClosureTable, ClosureTableBuildError, ClosureTransaction,
    StaleClosureTransaction,
};
use crate::semantic_authority::{
    SemanticAccess, SemanticAuthority, SemanticCommitError, SemanticTransaction,
};

/// One finalized checked generation of structural types, copy facts, and closure definitions.
///
/// This owner is constructed only by finishing [`BodySemanticAuthority`]. Closure types contain
/// `ClosureId`, so no other responsibility can pair a closure table with an unrelated semantic
/// generation.
#[derive(Debug)]
pub(crate) struct CheckedSemanticAuthority {
    semantics: SemanticAuthority,
    closures: ClosureTable,
}

impl CheckedSemanticAuthority {
    pub(crate) const fn semantics(&self) -> &SemanticAuthority {
        &self.semantics
    }

    pub(crate) const fn closures(&self) -> &ClosureTable {
        &self.closures
    }

    pub(crate) fn transaction(&self) -> SemanticTransaction {
        self.semantics.transaction()
    }

    pub(crate) fn accept(&mut self, transaction: SemanticTransaction) {
        self.semantics = transaction
            .commit(&self.semantics)
            .expect("checked transaction must commit to its exact semantic authority");
    }

    pub(crate) fn retain_recovery_branch(&mut self, transaction: SemanticTransaction) {
        self.semantics = transaction.freeze();
    }
}

/// One accepted generation of every semantic authority extended during body construction.
#[derive(Clone)]
pub(super) struct BodySemanticAuthority {
    semantics: SemanticAuthority,
    closures: ClosureAuthority,
}

impl BodySemanticAuthority {
    pub(super) const fn new(semantics: SemanticAuthority, closures: ClosureAuthority) -> Self {
        Self {
            semantics,
            closures,
        }
    }

    pub(super) fn transaction(&self) -> BodySemanticTransaction {
        BodySemanticTransaction {
            semantics: self.semantics.transaction(),
            closures: self.closures.transaction(),
        }
    }

    pub(super) fn finish_checked(self) -> Result<CheckedSemanticAuthority, ClosureTableBuildError> {
        Ok(CheckedSemanticAuthority {
            semantics: self.semantics,
            closures: self.closures.finish()?,
        })
    }

    pub(super) fn finish_recovery(self) -> SemanticAuthority {
        self.semantics
    }

    pub(super) const fn semantics(&self) -> &SemanticAuthority {
        &self.semantics
    }

    pub(super) const fn closures(&self) -> &ClosureAuthority {
        &self.closures
    }
}

/// The sole owner of program-wide semantic additions made while checking one body.
pub(super) struct BodySemanticTransaction {
    semantics: SemanticTransaction,
    closures: ClosureTransaction,
}

impl BodySemanticTransaction {
    pub(super) fn access(&mut self) -> BodySemanticAccess<'_> {
        BodySemanticAccess {
            semantics: self.semantics.access(),
            closures: &mut self.closures,
        }
    }

    pub(super) fn types_mut(&mut self) -> &mut nocter_model::TypeTransaction {
        self.semantics.types_mut()
    }

    pub(super) fn closures_mut(&mut self) -> &mut ClosureTransaction {
        &mut self.closures
    }

    pub(super) fn replay_parts(
        &mut self,
    ) -> (
        &mut nocter_model::TypeTransaction,
        &mut crate::copyability::CopyabilityTransaction,
        &mut ClosureTransaction,
    ) {
        let (types, copyabilities) = self.semantics.access().into_reasoning_parts();
        (types, copyabilities, &mut self.closures)
    }

    pub(super) fn commit(
        self,
        base: &BodySemanticAuthority,
    ) -> Result<BodySemanticAuthority, BodySemanticCommitError> {
        let semantics = self.semantics.commit(&base.semantics)?;
        let closures = self.closures.commit(&base.closures)?;
        Ok(BodySemanticAuthority {
            semantics,
            closures,
        })
    }

    pub(super) fn freeze_recovery(self) -> SemanticAuthority {
        self.semantics.freeze()
    }
}

pub(super) struct BodySemanticAccess<'authority> {
    semantics: SemanticAccess<'authority>,
    closures: &'authority mut ClosureTransaction,
}

impl<'authority> BodySemanticAccess<'authority> {
    pub(super) fn closures(&mut self) -> &mut ClosureTransaction {
        self.closures
    }

    pub(super) fn into_checker_parts(
        self,
    ) -> (
        &'authority mut nocter_model::TypeTransaction,
        &'authority mut crate::copyability::CopyabilityTransaction,
        &'authority mut ClosureTransaction,
    ) {
        let (types, copyabilities) = self.semantics.into_reasoning_parts();
        (types, copyabilities, self.closures)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BodySemanticCommitError {
    Semantics(SemanticCommitError),
    Closures(StaleClosureTransaction),
}

impl From<SemanticCommitError> for BodySemanticCommitError {
    fn from(error: SemanticCommitError) -> Self {
        Self::Semantics(error)
    }
}

impl From<StaleClosureTransaction> for BodySemanticCommitError {
    fn from(error: StaleClosureTransaction) -> Self {
        Self::Closures(error)
    }
}

#[cfg(test)]
mod tests {
    use crate::checked::ClosureAuthority;
    use crate::semantic_authority::SemanticAuthority;

    use super::BodySemanticAuthority;

    #[test]
    fn sibling_body_transaction_cannot_commit_after_authorities_advance() {
        let base =
            BodySemanticAuthority::new(SemanticAuthority::default(), ClosureAuthority::new());
        let first = base.transaction();
        let second = base.transaction();

        let accepted = first.commit(&base).unwrap();

        assert!(second.commit(&accepted).is_err());
    }
}
