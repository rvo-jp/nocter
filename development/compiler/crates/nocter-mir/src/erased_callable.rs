use nocter_model::{CallableCapability, ExecutableItemId, MirValueId, TypeId};

use crate::MirDestructionPlan;

/// One closed runtime construction of an owning erased callable.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirErasedCallable {
    environment: MirValueId,
    environment_ty: TypeId,
    body: ExecutableItemId,
    capability: CallableCapability,
    environment_destruction: Option<MirDestructionPlan>,
}

impl MirErasedCallable {
    #[must_use]
    pub fn new(
        environment: MirValueId,
        environment_ty: TypeId,
        body: ExecutableItemId,
        capability: CallableCapability,
        environment_destruction: Option<MirDestructionPlan>,
    ) -> Self {
        Self {
            environment,
            environment_ty,
            body,
            capability,
            environment_destruction,
        }
    }

    #[must_use]
    pub const fn environment(&self) -> MirValueId {
        self.environment
    }

    #[must_use]
    pub const fn environment_ty(&self) -> TypeId {
        self.environment_ty
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
    pub const fn environment_destruction(&self) -> Option<&MirDestructionPlan> {
        self.environment_destruction.as_ref()
    }
}
