use crate::{MachineFunctionId, MachineValueId};
use nocter_runtime_contract::RuntimeDataId;

/// Closed construction data for one owning erased callable value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MachineErasedCallable {
    environment: MachineValueId,
    environment_ty: RuntimeDataId,
    invoke: MachineFunctionId,
    destroy: Option<MachineFunctionId>,
}

impl MachineErasedCallable {
    pub(crate) const fn new(
        environment: MachineValueId,
        environment_ty: RuntimeDataId,
        invoke: MachineFunctionId,
        destroy: Option<MachineFunctionId>,
    ) -> Self {
        Self {
            environment,
            environment_ty,
            invoke,
            destroy,
        }
    }

    #[must_use]
    pub const fn environment(self) -> MachineValueId {
        self.environment
    }

    #[must_use]
    pub const fn environment_ty(self) -> RuntimeDataId {
        self.environment_ty
    }

    #[must_use]
    pub const fn invoke(self) -> MachineFunctionId {
        self.invoke
    }

    #[must_use]
    pub const fn destroy(self) -> Option<MachineFunctionId> {
        self.destroy
    }
}
