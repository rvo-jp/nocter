use std::fmt;
use std::ops::Deref;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{TypeId, TypeKind, TypeStore, UnknownTypeId};

static NEXT_TYPE_AUTHORITY: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Eq, PartialEq)]
struct TypeAuthorityIdentity(u64);

impl TypeAuthorityIdentity {
    fn fresh() -> Self {
        let identity = NEXT_TYPE_AUTHORITY
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .expect("type authority identity space exhausted");
        Self(identity)
    }
}

impl fmt::Debug for TypeAuthorityIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TypeAuthorityIdentity")
    }
}

/// Immutable ownership capability for one exact structural-type generation.
///
/// The contained [`TypeStore`] is the read contract. Only this authority can open or accept a
/// construction branch, so a consumer holding a `&TypeStore` cannot extend checked semantics.
#[derive(Clone)]
pub struct TypeAuthority {
    store: TypeStore,
    identity: TypeAuthorityIdentity,
}

impl TypeAuthority {
    #[must_use]
    pub fn new() -> Self {
        Self {
            store: TypeStore::new(),
            identity: TypeAuthorityIdentity::fresh(),
        }
    }

    #[must_use]
    pub const fn store(&self) -> &TypeStore {
        &self.store
    }

    #[must_use]
    pub fn transaction(&self) -> TypeTransaction {
        TypeTransaction {
            base: self.identity,
            branch: self.store.clone(),
        }
    }

    #[must_use]
    pub fn into_store(self) -> TypeStore {
        self.store
    }
}

impl Default for TypeAuthority {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for TypeAuthority {
    type Target = TypeStore;

    fn deref(&self) -> &Self::Target {
        &self.store
    }
}

impl fmt::Debug for TypeAuthority {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TypeAuthority")
            .field("type_count", &self.store.type_count())
            .field("identity", &self.identity)
            .finish()
    }
}

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
    /// Interns one structural type into this branch.
    ///
    /// # Errors
    ///
    /// Returns [`UnknownTypeId`] when `kind` refers to an identity outside this branch.
    pub fn intern(&mut self, kind: TypeKind) -> Result<TypeId, UnknownTypeId> {
        self.branch.intern_branch(kind)
    }

    /// Reports whether this branch was opened from the exact immutable authority supplied.
    ///
    /// Query-session owners use this check to reject accidental reuse across compiler
    /// generations without exposing the authority's lineage representation.
    #[must_use]
    pub fn is_based_on(&self, authority: &TypeAuthority) -> bool {
        self.base == authority.identity
    }

    /// Consumes this branch into an immutable descendant of `base`.
    ///
    /// # Errors
    ///
    /// Returns [`StaleTypeTransaction`] when `base` is not the exact authority from which this
    /// transaction was opened. In particular, a sibling's committed descendant cannot accept it.
    pub fn commit(self, base: &TypeAuthority) -> Result<TypeAuthority, StaleTypeTransaction> {
        if base.identity != self.base {
            return Err(StaleTypeTransaction);
        }
        Ok(TypeAuthority {
            store: self.branch,
            identity: TypeAuthorityIdentity::fresh(),
        })
    }

    /// Freezes the exact branch as an immutable recovery authority.
    ///
    /// Unlike commit, freezing deliberately does not require the current accepted base: rejected
    /// editor evidence retains this descendant without promoting it into compilation semantics.
    #[must_use]
    pub fn freeze(self) -> TypeAuthority {
        TypeAuthority {
            store: self.branch,
            identity: TypeAuthorityIdentity::fresh(),
        }
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
    use crate::{BuiltinType, TypeAuthority, TypeKind};

    #[test]
    fn commit_preserves_the_ancestor_and_freezes_the_descendant() {
        let base = TypeAuthority::new();
        let value = base.builtin(BuiltinType::I32);
        let mut transaction = base.transaction();
        let optional = transaction.intern(TypeKind::Optional(value)).unwrap();

        let descendant = transaction.commit(&base).unwrap();

        assert_eq!(base.get(optional), None);
        assert_eq!(descendant.get(optional), Some(&TypeKind::Optional(value)));
    }

    #[test]
    fn sibling_commit_is_rejected_after_the_accepted_authority_advances() {
        let base = TypeAuthority::new();
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
        let base = TypeAuthority::new();
        let value = base.builtin(BuiltinType::I32);
        let mut transaction = base.transaction();
        let provisional = transaction.intern(TypeKind::Optional(value)).unwrap();

        let recovery = transaction.freeze();

        assert_eq!(base.get(provisional), None);
        assert_eq!(recovery.get(provisional), Some(&TypeKind::Optional(value)));
    }

    #[test]
    fn base_compatibility_distinguishes_authorities_but_accepts_an_immutable_clone() {
        let base = TypeAuthority::new();
        let same = base.clone();
        let foreign = TypeAuthority::new();
        let transaction = base.transaction();

        assert!(transaction.is_based_on(&base));
        assert!(transaction.is_based_on(&same));
        assert!(!transaction.is_based_on(&foreign));
    }
}
