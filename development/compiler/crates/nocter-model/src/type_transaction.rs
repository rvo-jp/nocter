use std::fmt;
use std::ops::Deref;

use crate::type_store::TypeAuthorityIdentity;
use crate::{TypeId, TypeKind, TypeStore, UnknownTypeId};

/// A branch-local extension of one immutable type authority.
///
/// The transaction preserves every identity in its base and owns only the path-copied roots needed
/// by newly interned types. It cannot mutate its base. Committing consumes the transaction and
/// verifies that the caller still presents the exact authority from which the branch was opened.
#[derive(Debug)]
pub struct TypeTransaction {
    base: TypeAuthorityIdentity,
    branch: TypeStore,
}

impl TypeTransaction {
    pub(super) fn new(base: &TypeStore) -> Self {
        Self {
            base: base.authority(),
            branch: base.clone(),
        }
    }

    /// Interns one structural type into this branch.
    ///
    /// # Errors
    ///
    /// Returns [`UnknownTypeId`] when `kind` refers to an identity outside this branch.
    pub fn intern(&mut self, kind: TypeKind) -> Result<TypeId, UnknownTypeId> {
        self.branch.intern_branch(kind)
    }

    /// Consumes this branch into an immutable descendant of `base`.
    ///
    /// # Errors
    ///
    /// Returns [`StaleTypeTransaction`] when `base` is not the exact authority from which this
    /// transaction was opened. In particular, a sibling's committed descendant cannot accept it.
    pub fn commit(mut self, base: &TypeStore) -> Result<TypeStore, StaleTypeTransaction> {
        if base.authority() != self.base {
            return Err(StaleTypeTransaction);
        }
        self.branch.advance_authority();
        Ok(self.branch)
    }

    /// Freezes the exact branch as an immutable recovery authority.
    ///
    /// Unlike commit, freezing deliberately does not require the current accepted base: rejected
    /// editor evidence retains this descendant without promoting it into compilation semantics.
    #[must_use]
    pub fn freeze(mut self) -> TypeStore {
        self.branch.advance_authority();
        self.branch
    }
}

impl Deref for TypeTransaction {
    type Target = TypeStore;

    fn deref(&self) -> &Self::Target {
        &self.branch
    }
}

/// A transaction was offered to a foreign or already advanced authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaleTypeTransaction;

impl fmt::Display for StaleTypeTransaction {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("type transaction base is stale or belongs to another authority")
    }
}

impl std::error::Error for StaleTypeTransaction {}

#[cfg(test)]
mod tests {
    use crate::{BuiltinType, TypeKind, TypeStore};

    #[test]
    fn commit_preserves_the_ancestor_and_freezes_the_descendant() {
        let base = TypeStore::new();
        let value = base.builtin(BuiltinType::I32);
        let mut transaction = base.transaction();
        let optional = transaction.intern(TypeKind::Optional(value)).unwrap();

        let descendant = transaction.commit(&base).unwrap();

        assert_eq!(base.get(optional), None);
        assert_eq!(descendant.get(optional), Some(&TypeKind::Optional(value)));
    }

    #[test]
    fn sibling_commit_is_rejected_after_the_accepted_authority_advances() {
        let base = TypeStore::new();
        let value = base.builtin(BuiltinType::I32);
        let mut first = base.transaction();
        let mut second = base.transaction();
        first.intern(TypeKind::Optional(value)).unwrap();
        second.intern(TypeKind::Fallible(value)).unwrap();

        let accepted = first.commit(&base).unwrap();

        assert!(second.commit(&accepted).is_err());
    }

    #[test]
    fn frozen_recovery_isolated_from_its_base() {
        let base = TypeStore::new();
        let value = base.builtin(BuiltinType::I32);
        let mut transaction = base.transaction();
        let provisional = transaction.intern(TypeKind::Optional(value)).unwrap();

        let recovery = transaction.freeze();

        assert_eq!(base.get(provisional), None);
        assert_eq!(recovery.get(provisional), Some(&TypeKind::Optional(value)));
    }
}
