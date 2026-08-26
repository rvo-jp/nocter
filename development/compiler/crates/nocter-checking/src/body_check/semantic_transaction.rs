use crate::checked::{ClosureAuthority, ClosureTransaction, StaleClosureTransaction};
use crate::semantic_authority::{
    SemanticAccess, SemanticAuthority, SemanticCommitError, SemanticTransaction,
};

/// One accepted generation of every semantic authority extended during body construction.
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

    pub(super) fn finish(self) -> (SemanticAuthority, ClosureAuthority) {
        (self.semantics, self.closures)
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
        (
            self.semantics.types,
            self.semantics.copyabilities,
            self.closures,
        )
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
