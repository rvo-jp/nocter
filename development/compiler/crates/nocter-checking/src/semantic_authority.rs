use nocter_model::{StaleTypeTransaction, TypeAuthority, TypeStore, TypeTransaction};

use crate::checked::ClosureTable;
use crate::copyability::{CopyabilityTable, CopyabilityTransaction, StaleCopyabilityTransaction};

/// One immutable generation of the mutually dependent type and copyability authorities.
///
/// Copyability facts contain `TypeId` values, so exposing the two owners independently would allow
/// callers to pair facts with a sibling type branch. Prepared, recovery, and checked products keep
/// this value intact and expose only its read contracts.
#[derive(Clone, Debug)]
pub(crate) struct SemanticAuthority {
    types: TypeAuthority,
    copyabilities: CopyabilityTable,
}

impl SemanticAuthority {
    pub(crate) const fn seal(types: TypeAuthority, copyabilities: CopyabilityTable) -> Self {
        Self {
            types,
            copyabilities,
        }
    }

    pub(crate) const fn types(&self) -> &TypeStore {
        self.types.store()
    }

    #[cfg(test)]
    pub(crate) const fn type_authority(&self) -> &TypeAuthority {
        &self.types
    }

    pub(crate) const fn copyabilities(&self) -> &CopyabilityTable {
        &self.copyabilities
    }

    pub(crate) fn transaction(&self) -> SemanticTransaction {
        SemanticTransaction {
            types: self.types.transaction(),
            copyabilities: self.copyabilities.transaction(),
        }
    }
}

impl Default for SemanticAuthority {
    fn default() -> Self {
        Self::seal(TypeAuthority::new(), CopyabilityTable::default())
    }
}

#[derive(Debug)]
pub(crate) struct SemanticTransaction {
    types: TypeTransaction,
    copyabilities: CopyabilityTransaction,
}

impl SemanticTransaction {
    pub(crate) fn types(&self) -> &TypeStore {
        &self.types
    }

    pub(crate) fn types_mut(&mut self) -> &mut TypeTransaction {
        &mut self.types
    }

    pub(crate) fn is_based_on(&self, authority: &SemanticAuthority) -> bool {
        self.types.is_based_on(&authority.types)
            && self.copyabilities.is_based_on(&authority.copyabilities)
    }

    pub(crate) fn access(&mut self) -> SemanticAccess<'_> {
        SemanticAccess {
            types: &mut self.types,
            copyabilities: &mut self.copyabilities,
        }
    }

    pub(crate) fn commit(
        self,
        base: &SemanticAuthority,
    ) -> Result<SemanticAuthority, SemanticCommitError> {
        let types = self.types.commit(&base.types)?;
        let copyabilities = self.copyabilities.commit(&base.copyabilities)?;
        Ok(SemanticAuthority::seal(types, copyabilities))
    }

    pub(crate) fn freeze(self) -> SemanticAuthority {
        SemanticAuthority::seal(self.types.freeze(), self.copyabilities.freeze())
    }

    /// Closes specialization into the only semantic artifact consumed by executable construction.
    /// Copyability remains a checking-time proof cache and is deliberately not exported.
    pub(crate) fn finish_specialized_types(self) -> TypeStore {
        self.types.freeze().into_store()
    }
}

pub(crate) struct SemanticAccess<'authority> {
    types: &'authority mut TypeTransaction,
    copyabilities: &'authority mut CopyabilityTransaction,
}

impl<'authority> SemanticAccess<'authority> {
    pub(crate) fn into_reasoning_parts(
        self,
    ) -> (
        &'authority mut TypeTransaction,
        &'authority mut CopyabilityTransaction,
    ) {
        (self.types, self.copyabilities)
    }
}

/// One finalized checked generation of structural types, copy facts, and closure definitions.
///
/// Closure types contain `ClosureId`, so checked products retain this composite instead of accepting
/// a closure table independently from the semantic generation that created those identities.
#[derive(Debug)]
pub(crate) struct CheckedSemanticAuthority {
    semantics: SemanticAuthority,
    closures: ClosureTable,
}

impl CheckedSemanticAuthority {
    pub(crate) const fn new(semantics: SemanticAuthority, closures: ClosureTable) -> Self {
        Self {
            semantics,
            closures,
        }
    }

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SemanticCommitError {
    Types(StaleTypeTransaction),
    Copyabilities(StaleCopyabilityTransaction),
}

impl From<StaleTypeTransaction> for SemanticCommitError {
    fn from(error: StaleTypeTransaction) -> Self {
        Self::Types(error)
    }
}

impl From<StaleCopyabilityTransaction> for SemanticCommitError {
    fn from(error: StaleCopyabilityTransaction) -> Self {
        Self::Copyabilities(error)
    }
}

#[cfg(test)]
mod tests {
    use nocter_model::TypeAuthority;

    use crate::copyability::CopyabilityTable;

    use super::{SemanticAuthority, SemanticCommitError};

    #[test]
    fn a_transaction_rejects_a_mixed_component_generation() {
        let base = SemanticAuthority::seal(TypeAuthority::new(), CopyabilityTable::default());
        let transaction = base.transaction();
        let foreign_copyabilities = CopyabilityTable::default();
        let mixed = SemanticAuthority::seal(base.types.clone(), foreign_copyabilities);

        assert!(matches!(
            transaction.commit(&mixed),
            Err(SemanticCommitError::Copyabilities(_))
        ));
    }
}
