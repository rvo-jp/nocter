use nocter_runtime_contract::{PrimitiveBinding, RuntimeStorageBinding, TargetServiceBinding};

/// Source-neutral semantic bindings for every compiler-owned runtime catalog.
#[derive(Debug)]
pub(crate) struct RuntimeBindings {
    primitives: Box<[PrimitiveBinding]>,
    target_services: Box<[TargetServiceBinding]>,
    storage: Box<[RuntimeStorageBinding]>,
}

impl RuntimeBindings {
    #[must_use]
    pub(crate) const fn new(
        primitives: Box<[PrimitiveBinding]>,
        target_services: Box<[TargetServiceBinding]>,
        storage: Box<[RuntimeStorageBinding]>,
    ) -> Self {
        Self {
            primitives,
            target_services,
            storage,
        }
    }

    #[must_use]
    pub(crate) const fn primitives(&self) -> &[PrimitiveBinding] {
        &self.primitives
    }

    #[must_use]
    pub(crate) const fn target_services(&self) -> &[TargetServiceBinding] {
        &self.target_services
    }

    #[must_use]
    pub(crate) const fn storage(&self) -> &[RuntimeStorageBinding] {
        &self.storage
    }
}
