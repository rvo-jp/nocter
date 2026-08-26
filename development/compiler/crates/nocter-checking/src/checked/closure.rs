use std::fmt;

use nocter_model::{
    Arena, ArenaBuilder, ArenaCheckpoint, BodyId, BodyNodeId, CallableCapability, CallableContract,
    CaptureId, ClosureId, LocalBindingId, TypeId,
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

pub(crate) struct ClosureTableBuilder {
    slots: ArenaBuilder<ClosureId, ClosureSlot>,
    transaction: Option<Vec<ClosureMutation>>,
}

impl ClosureTableBuilder {
    pub(crate) fn new() -> Self {
        Self {
            slots: ArenaBuilder::new(),
            transaction: None,
        }
    }

    pub(crate) fn reserve(&mut self, owner: BodyId) -> ClosureId {
        self.slots.insert(ClosureSlot::Reserved(owner))
    }

    pub(crate) fn checkpoint(&mut self) -> ClosureTableCheckpoint {
        assert!(
            self.transaction.is_none(),
            "closure-table transactions cannot overlap"
        );
        self.transaction = Some(Vec::new());
        ClosureTableCheckpoint {
            slots: self.slots.checkpoint(),
            transaction: ClosureTransactionToken(std::marker::PhantomData),
        }
    }

    pub(crate) fn commit(&mut self, checkpoint: ClosureTableCheckpoint) {
        let ClosureTableCheckpoint {
            transaction: _transaction,
            ..
        } = checkpoint;
        self.transaction
            .take()
            .expect("closure-table transaction must be active before commit");
    }

    pub(crate) fn rollback(&mut self, checkpoint: ClosureTableCheckpoint) {
        let ClosureTableCheckpoint {
            slots,
            transaction: _transaction,
        } = checkpoint;
        let mutations = self
            .transaction
            .take()
            .expect("closure-table transaction must be active before rollback");
        for mutation in mutations.into_iter().rev() {
            match mutation {
                ClosureMutation::Definition { closure, owner } => {
                    let slot = self
                        .slots
                        .get_mut(closure)
                        .expect("mutated closure must remain until arena rollback");
                    *slot = ClosureSlot::Reserved(owner);
                }
                ClosureMutation::CallableRequirement {
                    closure,
                    previous_len,
                } => {
                    let Some(ClosureSlot::Defined(definition)) = self.slots.get_mut(closure) else {
                        panic!("callable-requirement mutation must name a defined closure");
                    };
                    definition.callable_requirements.truncate(previous_len);
                }
            }
        }
        self.slots.rollback(slots);
    }

    pub(crate) fn define(
        &mut self,
        closure: ClosureId,
        definition: ClosureDefinition,
    ) -> Result<(), ClosureTableBuildError> {
        let slot = self
            .slots
            .get_mut(closure)
            .ok_or(ClosureTableBuildError::UnknownClosure(closure))?;
        let ClosureSlot::Reserved(owner) = slot else {
            return Err(ClosureTableBuildError::DuplicateClosure(closure));
        };
        if *owner != definition.owner() {
            return Err(ClosureTableBuildError::OwnerMismatch(closure));
        }
        if let Some(transaction) = &mut self.transaction {
            transaction.push(ClosureMutation::Definition {
                closure,
                owner: *owner,
            });
        }
        *slot = ClosureSlot::Defined(definition);
        Ok(())
    }

    pub(crate) fn get(&self, closure: ClosureId) -> Option<&ClosureDefinition> {
        match self.slots.get(closure) {
            Some(ClosureSlot::Defined(definition)) => Some(definition),
            Some(ClosureSlot::Reserved(_)) | None => None,
        }
    }

    pub(crate) fn require_callable(
        &mut self,
        owner: BodyId,
        closure: ClosureId,
        contract: CallableContract,
    ) -> Result<(), ClosureTableBuildError> {
        let slot = self
            .slots
            .get_mut(closure)
            .ok_or(ClosureTableBuildError::UnknownClosure(closure))?;
        let ClosureSlot::Defined(definition) = slot else {
            return Err(ClosureTableBuildError::IncompleteClosure(closure));
        };
        if definition.owner() != owner {
            return Err(ClosureTableBuildError::OwnerMismatch(closure));
        }
        if !definition.callable_requirements.contains(&contract) {
            if let Some(transaction) = &mut self.transaction {
                transaction.push(ClosureMutation::CallableRequirement {
                    closure,
                    previous_len: definition.callable_requirements.len(),
                });
            }
            definition.callable_requirements.push(contract);
        }
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<ClosureTable, ClosureTableBuildError> {
        let definitions = self.slots.try_finish_with(|closure, slot| match slot {
            ClosureSlot::Reserved(_) => Err(ClosureTableBuildError::IncompleteClosure(closure)),
            ClosureSlot::Defined(definition) => Ok(definition),
        })?;
        Ok(ClosureTable::new(definitions))
    }
}

#[derive(Clone)]
enum ClosureSlot {
    Reserved(BodyId),
    Defined(ClosureDefinition),
}

pub(crate) struct ClosureTableCheckpoint {
    slots: ArenaCheckpoint<ClosureId, ClosureSlot>,
    transaction: ClosureTransactionToken,
}

struct ClosureTransactionToken(std::marker::PhantomData<std::cell::Cell<()>>);

enum ClosureMutation {
    Definition {
        closure: ClosureId,
        owner: BodyId,
    },
    CallableRequirement {
        closure: ClosureId,
        previous_len: usize,
    },
}

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

    use super::{ClosureDefinition, ClosureSignature, ClosureTableBuilder};

    #[test]
    fn reservation_fixes_identity_before_a_nested_body_is_defined() {
        let mut bodies = ArenaBuilder::<BodyId, _>::new();
        let owner = bodies.insert(());
        let _ = bodies.finish();
        let mut nodes = ArenaBuilder::<BodyNodeId, _>::new();
        let root = nodes.insert(());
        let _ = nodes.finish();
        let mut types = TypeStore::new();
        let mut closures = ClosureTableBuilder::new();
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

        let table = closures.finish().unwrap();
        assert_eq!(table.get(closure).unwrap().ty(), ty);
    }

    #[test]
    fn rollback_restores_mutated_preexisting_closure_state() {
        let mut bodies = ArenaBuilder::<BodyId, _>::new();
        let owner = bodies.insert(());
        let mut nodes = ArenaBuilder::<BodyNodeId, _>::new();
        let root = nodes.insert(());
        let mut types = TypeStore::new();
        let mut closures = ClosureTableBuilder::new();
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
        let checkpoint = closures.checkpoint();

        closures.require_callable(owner, closure, contract).unwrap();
        assert_eq!(
            closures.get(closure).unwrap().callable_requirements().len(),
            1
        );
        closures.rollback(checkpoint);

        assert!(
            closures
                .get(closure)
                .unwrap()
                .callable_requirements()
                .is_empty()
        );
    }
}
