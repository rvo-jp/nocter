use std::fmt;
use std::ops::Deref;

use nocter_model::{
    Arena, ArenaBuilder, BodyId, BodyNodeId, CallableCapability, CallableContract, CaptureId,
    ClosureId, LocalBindingId, PersistentArena, TypeId,
};

/// The fully typed invocation shape of one anonymous concrete closure.
///
/// Result provenance is deliberately absent. It is a program-wide inferred relation retained by
/// `ProvenanceTable`, not a fact guessed while the closure body is still being constructed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClosureSignature {
    capability: CallableCapability,
    parameters: Box<[ClosureParameter]>,
    result: TypeId,
}

/// One closure parameter whose local identity and checked type cannot diverge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClosureParameter {
    binding: LocalBindingId,
    ty: TypeId,
}

impl ClosureParameter {
    #[must_use]
    pub const fn new(binding: LocalBindingId, ty: TypeId) -> Self {
        Self { binding, ty }
    }

    #[must_use]
    pub const fn binding(self) -> LocalBindingId {
        self.binding
    }

    #[must_use]
    pub const fn ty(self) -> TypeId {
        self.ty
    }
}

impl ClosureSignature {
    pub(crate) fn new(
        capability: CallableCapability,
        parameters: impl Into<Box<[ClosureParameter]>>,
        result: TypeId,
    ) -> Self {
        Self {
            capability,
            parameters: parameters.into(),
            result,
        }
    }

    #[must_use]
    pub const fn capability(&self) -> CallableCapability {
        self.capability
    }

    #[must_use]
    pub const fn parameters(&self) -> &[ClosureParameter] {
        &self.parameters
    }

    pub fn parameter_types(&self) -> impl ExactSizeIterator<Item = TypeId> + '_ {
        self.parameters.iter().copied().map(ClosureParameter::ty)
    }

    #[must_use]
    pub const fn result(&self) -> TypeId {
        self.result
    }
}

/// One field stored in a checked closure environment.
///
/// `ty` describes the representation held by the closure value. For a borrow capture this is a
/// borrow type, not the type of the source place that the nested body reads through `binding`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClosureEnvironmentField {
    binding: CaptureId,
    ty: TypeId,
}

impl ClosureEnvironmentField {
    pub(crate) const fn new(binding: CaptureId, ty: TypeId) -> Self {
        Self { binding, ty }
    }

    #[must_use]
    pub const fn binding(self) -> CaptureId {
        self.binding
    }

    #[must_use]
    pub const fn ty(self) -> TypeId {
        self.ty
    }
}

/// One generated closure body and the exact environment bindings it operates on.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClosureDefinition {
    owner: BodyId,
    ty: TypeId,
    signature: ClosureSignature,
    environment: Box<[ClosureEnvironmentField]>,
    callable_requirements: Vec<CallableContract>,
    body: BodyNodeId,
}

impl ClosureDefinition {
    pub(crate) fn new(
        owner: BodyId,
        ty: TypeId,
        signature: ClosureSignature,
        environment: impl Into<Box<[ClosureEnvironmentField]>>,
        body: BodyNodeId,
    ) -> Self {
        Self {
            owner,
            ty,
            signature,
            environment: environment.into(),
            callable_requirements: Vec::new(),
            body,
        }
    }

    #[must_use]
    pub const fn owner(&self) -> BodyId {
        self.owner
    }

    #[must_use]
    pub const fn ty(&self) -> TypeId {
        self.ty
    }

    #[must_use]
    pub const fn signature(&self) -> &ClosureSignature {
        &self.signature
    }

    #[must_use]
    pub const fn environment(&self) -> &[ClosureEnvironmentField] {
        &self.environment
    }

    /// Concrete structural callable contracts this closure must satisfy at its use sites.
    ///
    /// Signature compatibility is checked when the edge is recorded. Result provenance is
    /// inferred later, so the provenance pass consumes the retained contracts without rebuilding
    /// generic-call selection.
    #[must_use]
    pub fn callable_requirements(&self) -> &[CallableContract] {
        &self.callable_requirements
    }

    #[must_use]
    pub const fn body(&self) -> BodyNodeId {
        self.body
    }
}

/// Sole checked-program authority for anonymous closure identity and generated body metadata.
#[derive(Debug)]
pub struct ClosureTable {
    definitions: Arena<ClosureId, ClosureDefinition>,
}

impl ClosureTable {
    const fn new(definitions: Arena<ClosureId, ClosureDefinition>) -> Self {
        Self { definitions }
    }

    #[must_use]
    pub const fn definitions(&self) -> &Arena<ClosureId, ClosureDefinition> {
        &self.definitions
    }

    #[must_use]
    pub fn get(&self, closure: ClosureId) -> Option<&ClosureDefinition> {
        self.definitions.get(closure)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ClosureAuthority {
    slots: PersistentArena<ClosureId, ClosureSlot>,
    authority: ClosureAuthorityIdentity,
}

impl ClosureAuthority {
    pub(crate) fn new() -> Self {
        Self {
            slots: PersistentArena::default(),
            authority: ClosureAuthorityIdentity::fresh(),
        }
    }

    pub(crate) fn transaction(&self) -> ClosureTransaction {
        ClosureTransaction {
            base: self.authority,
            branch: self.clone(),
        }
    }

    pub(crate) fn get(&self, closure: ClosureId) -> Option<&ClosureDefinition> {
        match self.slots.get(closure) {
            Some(ClosureSlot::Defined(definition)) => Some(definition),
            Some(ClosureSlot::Reserved(_)) | None => None,
        }
    }

    pub(crate) fn finish(self) -> Result<ClosureTable, ClosureTableBuildError> {
        let mut definitions = ArenaBuilder::new();
        for (closure, slot) in &self.slots {
            let ClosureSlot::Defined(definition) = slot else {
                return Err(ClosureTableBuildError::IncompleteClosure(closure));
            };
            let actual = definitions.insert(definition.clone());
            assert_eq!(
                actual, closure,
                "persistent closure order must preserve canonical identity"
            );
        }
        Ok(ClosureTable::new(definitions.finish()))
    }
}

#[derive(Debug)]
pub(crate) struct ClosureTransaction {
    base: ClosureAuthorityIdentity,
    branch: ClosureAuthority,
}

impl Deref for ClosureTransaction {
    type Target = ClosureAuthority;

    fn deref(&self) -> &Self::Target {
        &self.branch
    }
}

impl ClosureTransaction {
    pub(crate) fn reserve(&mut self, owner: BodyId) -> ClosureId {
        self.branch.slots.insert(ClosureSlot::Reserved(owner))
    }

    pub(crate) fn commit(
        mut self,
        base: &ClosureAuthority,
    ) -> Result<ClosureAuthority, StaleClosureTransaction> {
        if self.base != base.authority {
            return Err(StaleClosureTransaction);
        }
        self.branch.authority = ClosureAuthorityIdentity::fresh();
        Ok(self.branch)
    }

    pub(crate) fn define(
        &mut self,
        closure: ClosureId,
        definition: ClosureDefinition,
    ) -> Result<(), ClosureTableBuildError> {
        let slot = self
            .branch
            .slots
            .get(closure)
            .cloned()
            .ok_or(ClosureTableBuildError::UnknownClosure(closure))?;
        let ClosureSlot::Reserved(owner) = slot else {
            return Err(ClosureTableBuildError::DuplicateClosure(closure));
        };
        if owner != definition.owner() {
            return Err(ClosureTableBuildError::OwnerMismatch(closure));
        }
        self.branch
            .slots
            .replace(closure, ClosureSlot::Defined(definition))
            .map_err(|_| ClosureTableBuildError::UnknownClosure(closure))?;
        Ok(())
    }

    pub(crate) fn require_callable(
        &mut self,
        owner: BodyId,
        closure: ClosureId,
        contract: CallableContract,
    ) -> Result<(), ClosureTableBuildError> {
        let slot = self
            .branch
            .slots
            .get(closure)
            .cloned()
            .ok_or(ClosureTableBuildError::UnknownClosure(closure))?;
        let ClosureSlot::Defined(mut definition) = slot else {
            return Err(ClosureTableBuildError::IncompleteClosure(closure));
        };
        if definition.owner() != owner {
            return Err(ClosureTableBuildError::OwnerMismatch(closure));
        }
        if !definition.callable_requirements.contains(&contract) {
            definition.callable_requirements.push(contract);
            self.branch
                .slots
                .replace(closure, ClosureSlot::Defined(definition))
                .map_err(|_| ClosureTableBuildError::UnknownClosure(closure))?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
enum ClosureSlot {
    Reserved(BodyId),
    Defined(ClosureDefinition),
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct ClosureAuthorityIdentity(u64);

impl ClosureAuthorityIdentity {
    fn fresh() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT: AtomicU64 = AtomicU64::new(1);
        let identity = NEXT
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .expect("closure authority identity space exhausted");
        Self(identity)
    }
}

impl fmt::Debug for ClosureAuthorityIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ClosureAuthorityIdentity")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct StaleClosureTransaction;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClosureTableBuildError {
    UnknownClosure(ClosureId),
    DuplicateClosure(ClosureId),
    OwnerMismatch(ClosureId),
    IncompleteClosure(ClosureId),
}

impl fmt::Display for ClosureTableBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid closure-table construction: {self:?}")
    }
}

impl std::error::Error for ClosureTableBuildError {}

#[cfg(test)]
mod tests {
    use nocter_model::{
        ArenaBuilder, BodyId, BodyNodeId, BuiltinType, CallableCapability, CallableContract,
        ResultProvenance, TypeKind, TypeStore,
    };

    use super::{ClosureAuthority, ClosureDefinition, ClosureSignature};

    #[test]
    fn reservation_fixes_identity_before_a_nested_body_is_defined() {
        let mut bodies = ArenaBuilder::<BodyId, _>::new();
        let owner = bodies.insert(());
        let _ = bodies.finish();
        let mut nodes = ArenaBuilder::<BodyNodeId, _>::new();
        let root = nodes.insert(());
        let _ = nodes.finish();
        let mut types = TypeStore::new().transaction();
        let base = ClosureAuthority::new();
        let mut closures = base.transaction();
        let closure = closures.reserve(owner);
        let ty = types
            .intern(TypeKind::Closure {
                definition: closure,
                arguments: Box::new([]),
            })
            .unwrap();
        closures
            .define(
                closure,
                ClosureDefinition::new(
                    owner,
                    ty,
                    ClosureSignature::new(
                        nocter_model::CallableCapability::Readonly,
                        [],
                        types.builtin(BuiltinType::Void),
                    ),
                    [],
                    root,
                ),
            )
            .unwrap();

        let table = closures.commit(&base).unwrap().finish().unwrap();
        assert_eq!(table.get(closure).unwrap().ty(), ty);
    }

    #[test]
    fn discarded_branch_cannot_mutate_a_preexisting_closure() {
        let mut bodies = ArenaBuilder::<BodyId, _>::new();
        let owner = bodies.insert(());
        let mut nodes = ArenaBuilder::<BodyNodeId, _>::new();
        let root = nodes.insert(());
        let mut types = TypeStore::new().transaction();
        let base = ClosureAuthority::new();
        let mut closures = base.transaction();
        let closure = closures.reserve(owner);
        let ty = types
            .intern(TypeKind::Closure {
                definition: closure,
                arguments: Box::new([]),
            })
            .unwrap();
        closures
            .define(
                closure,
                ClosureDefinition::new(
                    owner,
                    ty,
                    ClosureSignature::new(
                        CallableCapability::Readonly,
                        [],
                        types.builtin(BuiltinType::Void),
                    ),
                    [],
                    root,
                ),
            )
            .unwrap();
        let contract = CallableContract::new(
            CallableCapability::Readonly,
            [],
            None,
            types.builtin(BuiltinType::Void),
            ResultProvenance::empty(),
        )
        .unwrap();
        let accepted = closures.commit(&base).unwrap();
        let mut branch = accepted.transaction();

        branch.require_callable(owner, closure, contract).unwrap();
        assert_eq!(
            branch.get(closure).unwrap().callable_requirements().len(),
            1
        );
        drop(branch);

        assert!(
            accepted
                .get(closure)
                .unwrap()
                .callable_requirements()
                .is_empty()
        );
    }

    #[test]
    fn sibling_transaction_is_stale_after_authority_advances() {
        let base = ClosureAuthority::new();
        let first = base.transaction();
        let second = base.transaction();
        let accepted = first.commit(&base).unwrap();

        assert!(second.commit(&accepted).is_err());
    }
}
