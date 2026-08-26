use nocter_model::{StaleTypeTransaction, TypeStore, TypeTransaction};

use crate::checked::{ClosureAuthority, ClosureTransaction, StaleClosureTransaction};
use crate::copyability::{CopyabilityTable, CopyabilityTransaction, StaleCopyabilityTransaction};

/// The sole owner of program-wide semantic additions made while checking one body.
///
/// Components cannot be committed independently through this API. Success consumes all three
/// branches into coordinated immutable descendants; failure consumes the transaction into exact
/// recovery evidence or drops it without repairing any accepted authority.
pub(super) struct BodySemanticTransaction {
    types: TypeTransaction,
    copyabilities: CopyabilityTransaction,
    closures: ClosureTransaction,
}

impl BodySemanticTransaction {
    pub(super) fn new(
        types: &TypeStore,
        copyabilities: &CopyabilityTable,
        closures: &ClosureAuthority,
    ) -> Self {
        Self {
            types: types.transaction(),
            copyabilities: copyabilities.transaction(),
            closures: closures.transaction(),
        }
    }

    pub(super) fn parts(
        &mut self,
    ) -> (
        &mut TypeTransaction,
        &mut CopyabilityTransaction,
        &mut ClosureTransaction,
    ) {
        (&mut self.types, &mut self.copyabilities, &mut self.closures)
    }

    pub(super) fn into_parts(
        self,
    ) -> (TypeTransaction, CopyabilityTransaction, ClosureTransaction) {
        (self.types, self.copyabilities, self.closures)
    }

    pub(super) fn commit(
        self,
        types: &TypeStore,
        copyabilities: &CopyabilityTable,
        closures: &ClosureAuthority,
    ) -> Result<CommittedBodySemantic, BodySemanticCommitError> {
        let types = self.types.commit(types)?;
        let copyabilities = self.copyabilities.commit(copyabilities)?;
        let closures = self.closures.commit(closures)?;
        Ok(CommittedBodySemantic {
            types,
            copyabilities,
            closures,
        })
    }
}

pub(super) struct CommittedBodySemantic {
    pub(super) types: TypeStore,
    pub(super) copyabilities: CopyabilityTable,
    pub(super) closures: ClosureAuthority,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum BodySemanticCommitError {
    Types(StaleTypeTransaction),
    Copyabilities(StaleCopyabilityTransaction),
    Closures(StaleClosureTransaction),
}

impl From<StaleTypeTransaction> for BodySemanticCommitError {
    fn from(error: StaleTypeTransaction) -> Self {
        Self::Types(error)
    }
}

impl From<StaleCopyabilityTransaction> for BodySemanticCommitError {
    fn from(error: StaleCopyabilityTransaction) -> Self {
        Self::Copyabilities(error)
    }
}

impl From<StaleClosureTransaction> for BodySemanticCommitError {
    fn from(error: StaleClosureTransaction) -> Self {
        Self::Closures(error)
    }
}

#[cfg(test)]
mod tests {
    use nocter_model::TypeStore;

    use crate::checked::ClosureAuthority;
    use crate::copyability::CopyabilityTable;

    use super::BodySemanticTransaction;

    #[test]
    fn sibling_body_transaction_cannot_commit_after_authorities_advance() {
        let types = TypeStore::new();
        let copyabilities = CopyabilityTable::default();
        let closures = ClosureAuthority::new();
        let first = BodySemanticTransaction::new(&types, &copyabilities, &closures);
        let second = BodySemanticTransaction::new(&types, &copyabilities, &closures);

        let accepted = first.commit(&types, &copyabilities, &closures).unwrap();

        assert!(
            second
                .commit(&accepted.types, &accepted.copyabilities, &accepted.closures,)
                .is_err()
        );
    }
}
