use nocter_checking::ConcreteDestructionPlan;
use nocter_model::{BodyNodeId, CallableCapability, ExecutableItemId, TypeId};

/// One fully specialized closure-to-erased-callable conversion.
///
/// Executable closure owns this descriptor so MIR lowering never reopens closure identity,
/// generic substitution, or destruction selection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableErasedCallable {
    source: BodyNodeId,
    ty: TypeId,
    environment: TypeId,
    body: ExecutableItemId,
    capability: CallableCapability,
    source_capability: CallableCapability,
    environment_destruction: Option<ConcreteDestructionPlan>,
}

impl ExecutableErasedCallable {
    pub(super) const fn new(
        source: BodyNodeId,
        ty: TypeId,
        environment: TypeId,
        body: ExecutableItemId,
        capability: CallableCapability,
        source_capability: CallableCapability,
        environment_destruction: Option<ConcreteDestructionPlan>,
    ) -> Self {
        Self {
            source,
            ty,
            environment,
            body,
            capability,
            source_capability,
            environment_destruction,
        }
    }

    #[must_use]
    pub const fn source(&self) -> BodyNodeId {
        self.source
    }

    #[must_use]
    pub const fn ty(&self) -> TypeId {
        self.ty
    }

    #[must_use]
    pub const fn environment(&self) -> TypeId {
        self.environment
    }

    #[must_use]
    pub const fn body(&self) -> ExecutableItemId {
        self.body
    }

    #[must_use]
    pub const fn capability(&self) -> CallableCapability {
        self.capability
    }

    #[must_use]
    pub const fn source_capability(&self) -> CallableCapability {
        self.source_capability
    }

    #[must_use]
    pub const fn environment_destruction(&self) -> Option<&ConcreteDestructionPlan> {
        self.environment_destruction.as_ref()
    }
}
