use std::fmt;

use nocter_model::{
    Arena, ArenaBuilder, BodyId, BodyNodeId, CallableCapability, CallableContract, CaptureId,
    ClosureId, LocalBindingId, TypeId,
};

/// The fully typed invocation shape of one anonymous concrete closure.
///
/// Result provenance is deliberately absent. It is a program-wide inferred relation retained by
/// `ProvenanceTable`, not a fact guessed while the closure body is still being constructed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClosureSignature {
    capability: CallableCapability,
    parameters: Box<[TypeId]>,
    result: TypeId,
}

impl ClosureSignature {
    pub(crate) fn new(
        capability: CallableCapability,
        parameters: impl Into<Box<[TypeId]>>,
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
    pub const fn parameters(&self) -> &[TypeId] {
        &self.parameters
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
    parameters: Box<[LocalBindingId]>,
    environment: Box<[ClosureEnvironmentField]>,
    callable_requirements: Vec<CallableContract>,
    body: BodyNodeId,
}

impl ClosureDefinition {
    pub(crate) fn new(
        owner: BodyId,
        ty: TypeId,
        signature: ClosureSignature,
        parameters: impl Into<Box<[LocalBindingId]>>,
        environment: impl Into<Box<[ClosureEnvironmentField]>>,
        body: BodyNodeId,
    ) -> Self {
        Self {
            owner,
            ty,
            signature,
            parameters: parameters.into(),
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
    pub const fn parameters(&self) -> &[LocalBindingId] {
        &self.parameters
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

#[derive(Clone)]
pub(crate) struct ClosureTableBuilder {
    slots: ArenaBuilder<ClosureId, ClosureSlot>,
}

impl ClosureTableBuilder {
    pub(crate) fn new() -> Self {
        Self {
            slots: ArenaBuilder::new(),
        }
    }

    pub(crate) fn reserve(&mut self, owner: BodyId) -> ClosureId {
        self.slots.insert(ClosureSlot::Reserved(owner))
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
        if !definition.callable_requirements.contains(&contract) {
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
    use nocter_model::{ArenaBuilder, BodyId, BodyNodeId, BuiltinType, TypeKind, TypeStore};

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
                    [],
                    root,
                ),
            )
            .unwrap();

        let table = closures.finish().unwrap();
        assert_eq!(table.get(closure).unwrap().ty(), ty);
    }
}
