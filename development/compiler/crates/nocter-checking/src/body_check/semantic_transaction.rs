use nocter_model::{StaleTypeTransaction, TypeStore, TypeTransaction};

use crate::checked::{ClosureAuthority, ClosureTransaction, StaleClosureTransaction};
use crate::copyability::{CopyabilityTable, CopyabilityTransaction, StaleCopyabilityTransaction};

/// One accepted generation of every semantic authority extended during body construction.
///
/// Keeping the three components private prevents callers from pairing a type branch with
/// copyability or closure state from another body generation.
pub(super) struct BodySemanticAuthority {
    types: TypeStore,
    copyabilities: CopyabilityTable,
    closures: ClosureAuthority,
}

impl BodySemanticAuthority {
    pub(super) fn new(
        types: TypeStore,
        copyabilities: CopyabilityTable,
        closures: ClosureAuthority,
    ) -> Self {
        Self {
            types,
            copyabilities,
            closures,
        }
    }

    pub(super) fn transaction(&self) -> BodySemanticTransaction {
        BodySemanticTransaction {
            types: self.types.transaction(),
            copyabilities: self.copyabilities.transaction(),
            closures: self.closures.transaction(),
        }
    }

    pub(super) fn into_parts(self) -> (TypeStore, CopyabilityTable, ClosureAuthority) {
        (self.types, self.copyabilities, self.closures)
    }
}

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
        base: &BodySemanticAuthority,
    ) -> Result<BodySemanticAuthority, BodySemanticCommitError> {
        let types = self.types.commit(&base.types)?;
        let copyabilities = self.copyabilities.commit(&base.copyabilities)?;
        let closures = self.closures.commit(&base.closures)?;
        Ok(BodySemanticAuthority {
            types,
            copyabilities,
            closures,
        })
    }
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

    use super::BodySemanticAuthority;

    #[test]
    fn sibling_body_transaction_cannot_commit_after_authorities_advance() {
        let base = BodySemanticAuthority::new(
            TypeStore::new(),
            CopyabilityTable::default(),
            ClosureAuthority::new(),
        );
        let first = base.transaction();
        let second = base.transaction();

        let accepted = first.commit(&base).unwrap();

        assert!(second.commit(&accepted).is_err());
    }
}
